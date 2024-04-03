use num_derive::{FromPrimitive, ToPrimitive};

pub const MAX_STR_LEN: usize = 256;

pub const INIT_PASSWD: u64 = 0x4e42444d41474943;
pub const CLISERV_MAGIC: u64 = 0x00420281861253;
pub const IHAVEOPT: u64 = 0x49484156454F5054;
pub const NBD_REQUEST_MAGIC: u32 = 0x25609513;
pub const NBD_OPT_REPLY_MAGIC: u64 = 0x3e889045565a9;

pub const NBD_NEWSTYLE_PORT: u16 = 10809;

bitflags::bitflags! {
    // Handshake flags:
    // https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md#handshake-flags
    #[derive(Debug, Clone, Copy)]
    pub struct NbdHandshakeFlag: u16 {
        const FIXED_NEWSTYLE    = 0x0001;
        const NO_ZEROES         = 0x0002;
    }

    // Client flags:
    // https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md#client-flags
    #[derive(Debug, Clone, Copy)]
    pub struct NbdClientFlag: u32 {
        const FIXED_NEWSTYLE    = 0x0001;
        const NO_ZEROES         = 0x0002;
    }

    // Transmission flags:
    // https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md#transmission-flags
    #[derive(Debug, Clone, Copy)]
    pub struct NbdTransFlag: u16 {
        const HAS_FLAGS         = 0x0001;
        const READ_ONLY         = 0x0002;
        const SEND_FLUSH        = 0x0004;
        const SEND_FUA          = 0x0008;
        const SEND_ROTATIONAL   = 0x0010;
        const SEND_TRIM         = 0x0020;
        const SEND_WRITE_ZEROES = 0x0040;
        const SEND_DF           = 0x0080;
        const CAN_MULTI_CONN    = 0x0100;
        const SEND_RESIZE       = 0x0200;
        const SEND_CACHE        = 0x0400;
        const SEND_FAST_ZERO    = 0x0800;
        const BLOCK_STATUS_PAYLOAD  = 0x1000;
    }
}

// Option types:
// https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md#option-types
#[repr(u32)]
#[derive(Debug, Clone, Copy, FromPrimitive, ToPrimitive, PartialEq, Eq)]
pub enum NbdOpt {
    ExportName = 1,
    Abort = 2,
    List = 3,
    PeekExport = 4,
    Starttls = 5,
    Info = 6,
    Go = 7,
    StructuredReply = 8,
    ListMetaContext = 9,
    SetMetaContext = 10,
    ExtendedHeaders = 11,
}

// Option reply types:
// https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md#option-reply-types
#[repr(i32)]
#[derive(Debug, Clone, Copy, FromPrimitive, ToPrimitive, PartialEq, Eq)]
pub enum NbdOptReply {
    Ack = 1,
    Server = 2,
    Info = 3,
    MetaContext = 4,

    ErrUnsup = -1,
    ErrPolicy = -2,
    ErrInvalid = -3,
    ErrPlatform = -4,
    ErrTlsReqd = -5,
    ErrUnknown = -6,
    ErrShutdown = -7,
    ErrBlockSizeReqd = -8,
    ErrTooBig = -9,
    ErrExtHeaderReqd = -10,
}

// Rbd Info types.
#[repr(u16)]
#[derive(Debug, Clone, Copy, FromPrimitive, ToPrimitive, PartialEq, Eq)]
pub enum NbdInfo {
    Name = 1,
    Description = 2,
    BlockSize = 3,
}

// Request types:
// https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md#request-types
#[derive(Debug, Clone, Copy, FromPrimitive, ToPrimitive, PartialEq, Eq)]
#[repr(u16)]
pub enum NbdCmd {
    Read = 0,
    Write = 1,
    Disk = 2,
    Flush = 3,
    Trim = 4,
    Cache = 5,
    WriteZeroes = 6,
    BlockStatus = 7,
    Resize = 8,
}
