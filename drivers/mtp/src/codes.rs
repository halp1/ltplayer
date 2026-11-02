//! Operation, response, format and property codes from the PTP and MTP specs.

/// The only storage this responder exposes.
pub const STORAGE_ID: u32 = 0x0001_0001;

pub mod op {
  pub const GET_DEVICE_INFO: u16 = 0x1001;
  pub const OPEN_SESSION: u16 = 0x1002;
  pub const CLOSE_SESSION: u16 = 0x1003;
  pub const GET_STORAGE_IDS: u16 = 0x1004;
  pub const GET_STORAGE_INFO: u16 = 0x1005;
  pub const GET_NUM_OBJECTS: u16 = 0x1006;
  pub const GET_OBJECT_HANDLES: u16 = 0x1007;
  pub const GET_OBJECT_INFO: u16 = 0x1008;
  pub const GET_OBJECT: u16 = 0x1009;
  pub const DELETE_OBJECT: u16 = 0x100B;
  pub const SEND_OBJECT_INFO: u16 = 0x100C;
  pub const SEND_OBJECT: u16 = 0x100D;
  pub const GET_DEVICE_PROP_DESC: u16 = 0x1014;
  pub const GET_DEVICE_PROP_VALUE: u16 = 0x1015;
  pub const SET_DEVICE_PROP_VALUE: u16 = 0x1016;
  pub const MOVE_OBJECT: u16 = 0x1019;
  pub const GET_PARTIAL_OBJECT: u16 = 0x101B;
  pub const GET_OBJECT_PROPS_SUPPORTED: u16 = 0x9801;
  pub const GET_OBJECT_PROP_DESC: u16 = 0x9802;
  pub const GET_OBJECT_PROP_VALUE: u16 = 0x9803;
  pub const SET_OBJECT_PROP_VALUE: u16 = 0x9804;
  pub const GET_OBJECT_PROP_LIST: u16 = 0x9805;
  pub const SEND_OBJECT_PROP_LIST: u16 = 0x9808;

  pub const SUPPORTED: &[u16] = &[
    GET_DEVICE_INFO,
    OPEN_SESSION,
    CLOSE_SESSION,
    GET_STORAGE_IDS,
    GET_STORAGE_INFO,
    GET_NUM_OBJECTS,
    GET_OBJECT_HANDLES,
    GET_OBJECT_INFO,
    GET_OBJECT,
    DELETE_OBJECT,
    SEND_OBJECT_INFO,
    SEND_OBJECT,
    GET_DEVICE_PROP_DESC,
    GET_DEVICE_PROP_VALUE,
    MOVE_OBJECT,
    GET_PARTIAL_OBJECT,
    GET_OBJECT_PROPS_SUPPORTED,
    GET_OBJECT_PROP_DESC,
    GET_OBJECT_PROP_VALUE,
    SET_OBJECT_PROP_VALUE,
    GET_OBJECT_PROP_LIST,
  ];

  /// Operations where the host sends a data phase after the command.
  pub const fn host_sends_data(code: u16) -> bool {
    matches!(
      code,
      SEND_OBJECT_INFO
        | SEND_OBJECT
        | SET_OBJECT_PROP_VALUE
        | SET_DEVICE_PROP_VALUE
        | SEND_OBJECT_PROP_LIST
    )
  }
}

pub mod resp {
  pub const OK: u16 = 0x2001;
  pub const GENERAL_ERROR: u16 = 0x2002;
  pub const SESSION_NOT_OPEN: u16 = 0x2003;
  pub const OPERATION_NOT_SUPPORTED: u16 = 0x2005;
  pub const PARAMETER_NOT_SUPPORTED: u16 = 0x2006;
  pub const INCOMPLETE_TRANSFER: u16 = 0x2007;
  pub const INVALID_STORAGE_ID: u16 = 0x2008;
  pub const INVALID_OBJECT_HANDLE: u16 = 0x2009;
  pub const DEVICE_PROP_NOT_SUPPORTED: u16 = 0x200A;
  pub const STORE_FULL: u16 = 0x200C;
  pub const ACCESS_DENIED: u16 = 0x200F;
  pub const NO_VALID_OBJECT_INFO: u16 = 0x2015;
  pub const INVALID_PARENT_OBJECT: u16 = 0x201A;
  pub const INVALID_PARAMETER: u16 = 0x201D;
  pub const SESSION_ALREADY_OPEN: u16 = 0x201E;
  pub const INVALID_OBJECT_PROP_CODE: u16 = 0xA801;
  pub const INVALID_OBJECT_PROP_VALUE: u16 = 0xA803;
  pub const INVALID_DATASET: u16 = 0xA806;
  pub const SPEC_BY_GROUP_UNSUPPORTED: u16 = 0xA807;
  pub const SPEC_BY_DEPTH_UNSUPPORTED: u16 = 0xA808;
  pub const OBJECT_TOO_LARGE: u16 = 0xA809;
}

pub mod format {
  pub const UNDEFINED: u16 = 0x3000;
  pub const ASSOCIATION: u16 = 0x3001;
  pub const WAV: u16 = 0x3008;
  pub const MP3: u16 = 0x3009;
  pub const OGG: u16 = 0xB902;
  pub const AAC: u16 = 0xB903;
  pub const FLAC: u16 = 0xB906;

  pub const SUPPORTED: &[u16] = &[UNDEFINED, ASSOCIATION, WAV, MP3, OGG, AAC, FLAC];
}

pub mod prop {
  pub const STORAGE_ID: u16 = 0xDC01;
  pub const OBJECT_FORMAT: u16 = 0xDC02;
  pub const PROTECTION_STATUS: u16 = 0xDC03;
  pub const OBJECT_SIZE: u16 = 0xDC04;
  pub const OBJECT_FILE_NAME: u16 = 0xDC07;
  pub const PARENT_OBJECT: u16 = 0xDC0B;
  pub const NAME: u16 = 0xDC44;

  pub const SUPPORTED: &[u16] = &[
    STORAGE_ID,
    OBJECT_FORMAT,
    PROTECTION_STATUS,
    OBJECT_SIZE,
    OBJECT_FILE_NAME,
    PARENT_OBJECT,
    NAME,
  ];
}

pub mod device_prop {
  pub const FRIENDLY_NAME: u16 = 0xD402;
}

pub mod dtype {
  pub const U16: u16 = 0x0004;
  pub const U32: u16 = 0x0006;
  pub const U64: u16 = 0x0008;
  pub const STR: u16 = 0xFFFF;
}
