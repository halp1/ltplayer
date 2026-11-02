//! MTP over USB with embassy-usb.
//!
//! Registers one still-image class interface (bulk IN, bulk OUT, interrupt
//! IN), answers the PTP class control requests, and adds an MS OS 2.0
//! compatible ID so Windows binds its MTP driver without an .inf file.

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use embassy_usb::control::{InResponse, OutResponse, Recipient, Request, RequestType};
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::msos::CompatibleIdFeatureDescriptor;
use embassy_usb::types::{InterfaceNumber, StringIndex};
use embassy_usb::{Builder, Handler};

use crate::codes::resp;
use crate::transport::Transport;

pub const MAX_PACKET: u16 = 64;

const CLASS_STILL_IMAGE: u8 = 0x06;
const SUBCLASS_STILL_IMAGE: u8 = 0x01;
const PROTOCOL_PTP: u8 = 0x01;

const REQ_CANCEL: u8 = 0x64;
const REQ_DEVICE_RESET: u8 = 0x66;
const REQ_GET_DEVICE_STATUS: u8 = 0x67;

/// Flags shared between the control handler and the bulk transport.
pub struct Control {
  iface: AtomicU8,
  string: AtomicU8,
  cancel: AtomicBool,
  reset: AtomicBool,
  /// Bus reset or detach since the responder last checked.
  bus_reset: AtomicBool,
}

/// Storage for the class. Must outlive the USB device, usually a `static`.
pub struct UsbState<'d> {
  control: Control,
  handler: MaybeUninit<ControlHandler<'d>>,
}

impl Default for UsbState<'_> {
  fn default() -> Self {
    Self::new()
  }
}

impl UsbState<'_> {
  pub const fn new() -> Self {
    Self {
      control: Control::new(),
      handler: MaybeUninit::uninit(),
    }
  }
}

impl Control {
  const fn new() -> Self {
    Self {
      iface: AtomicU8::new(0),
      string: AtomicU8::new(0),
      cancel: AtomicBool::new(false),
      reset: AtomicBool::new(false),
      bus_reset: AtomicBool::new(false),
    }
  }

  fn for_us(&self, req: &Request) -> bool {
    req.request_type == RequestType::Class
      && req.recipient == Recipient::Interface
      && req.index == self.iface.load(Ordering::Relaxed) as u16
  }

  /// True once after a USB reset, detach, or class Device Reset request.
  pub fn take_session_reset(&self) -> bool {
    self.bus_reset.swap(false, Ordering::AcqRel) | self.reset.swap(false, Ordering::AcqRel)
  }
}

struct ControlHandler<'d>(&'d Control);

impl Handler for ControlHandler<'_> {
  fn enabled(&mut self, enabled: bool) {
    if !enabled {
      self.0.bus_reset.store(true, Ordering::Release);
    }
  }

  fn reset(&mut self) {
    self.0.bus_reset.store(true, Ordering::Release);
  }

  fn configured(&mut self, configured: bool) {
    if !configured {
      self.0.bus_reset.store(true, Ordering::Release);
    }
  }

  fn control_out(&mut self, req: Request, _data: &[u8]) -> Option<OutResponse> {
    if !self.0.for_us(&req) {
      return None;
    }
    match req.request {
      REQ_CANCEL => {
        self.0.cancel.store(true, Ordering::Release);
        Some(OutResponse::Accepted)
      }
      REQ_DEVICE_RESET => {
        self.0.reset.store(true, Ordering::Release);
        Some(OutResponse::Accepted)
      }
      _ => Some(OutResponse::Rejected),
    }
  }

  fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
    if !self.0.for_us(&req) {
      return None;
    }
    match req.request {
      REQ_GET_DEVICE_STATUS if buf.len() >= 4 => {
        // A pending cancel reports "busy" until the bulk side has seen it.
        let code = if self.0.cancel.load(Ordering::Acquire) {
          resp::GENERAL_ERROR
        } else {
          resp::OK
        };
        buf[0..2].copy_from_slice(&4u16.to_le_bytes());
        buf[2..4].copy_from_slice(&code.to_le_bytes());
        Some(InResponse::Accepted(&buf[..4]))
      }
      _ => Some(InResponse::Rejected),
    }
  }

  fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&str> {
    (index.0 == self.0.string.load(Ordering::Relaxed)).then_some("MTP")
  }
}

/// Bulk endpoints of the MTP interface.
pub struct UsbTransport<'d, D: Driver<'d>> {
  read_ep: D::EndpointOut,
  write_ep: D::EndpointIn,
  _event_ep: D::EndpointIn,
  control: &'d Control,
}

impl<'d, D: Driver<'d>> UsbTransport<'d, D> {
  /// Add the MTP interface to `builder`. Call
  /// `builder.msos_descriptor(windows_version::WIN8_1, vendor_code)` first.
  pub fn new(builder: &mut Builder<'d, D>, state: &'d mut UsbState<'d>) -> Self {
    let mut func = builder.function(CLASS_STILL_IMAGE, SUBCLASS_STILL_IMAGE, PROTOCOL_PTP);
    func.msos_feature(CompatibleIdFeatureDescriptor::new("MTP", ""));
    let mut iface = func.interface();
    let number: InterfaceNumber = iface.interface_number();
    let string = iface.string();
    let mut alt = iface.alt_setting(
      CLASS_STILL_IMAGE,
      SUBCLASS_STILL_IMAGE,
      PROTOCOL_PTP,
      Some(string),
    );
    let read_ep = alt.endpoint_bulk_out(None, MAX_PACKET);
    let write_ep = alt.endpoint_bulk_in(None, MAX_PACKET);
    let event_ep = alt.endpoint_interrupt_in(None, 28, 10);
    drop(func);

    let control = &state.control;
    control.iface.store(number.0, Ordering::Relaxed);
    control.string.store(string.0, Ordering::Relaxed);
    builder.handler(state.handler.write(ControlHandler(control)));

    Self {
      read_ep,
      write_ep,
      _event_ep: event_ep,
      control,
    }
  }

  pub fn control(&self) -> &'d Control {
    self.control
  }

  pub async fn wait_connected(&mut self) {
    self.read_ep.wait_enabled().await;
  }
}

impl<'d, D: Driver<'d>> Transport for UsbTransport<'d, D> {
  type Error = EndpointError;
  const MAX_PACKET: usize = MAX_PACKET as usize;

  async fn read_packet(&mut self, buf: &mut [u8]) -> Result<usize, EndpointError> {
    self.read_ep.read(buf).await
  }

  async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
    self.write_ep.write(data).await
  }

  fn take_cancel(&mut self) -> bool {
    self.control.cancel.swap(false, Ordering::AcqRel)
  }
}
