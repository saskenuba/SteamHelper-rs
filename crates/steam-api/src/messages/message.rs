//! Message Module
//!
//! Now that the message has been decoded, we can check what message is being sent to us.
//! It may be a protobuf message, or not.
//!
//!
//! Check link below for more info:
//! https://github.com/ValvePython/steam/blob/09f4f51a287ee7aec1f159c7e8098add5f14bed3/steam/core/msg/headers.py

//  if message is proto: emsg_enum, raw_data from packet
// new MessageHeaderProtobuf
// steammessages_base_pb2. CMSGProtobufHeader

//  if not proto: emsg_enum, raw_data from packet -> extender
// novo  ExtendedMessageHeader
