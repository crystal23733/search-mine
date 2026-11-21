#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoomCode([u8; 8]);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidRoomCode;
impl RoomCode {
    pub fn parse(value: &str) -> Result<Self, InvalidRoomCode> {
        if !liar_protocol::rooms::valid_room_code(value) {
            return Err(InvalidRoomCode);
        }
        let bytes = value.as_bytes().try_into().map_err(|_| InvalidRoomCode)?;
        Ok(Self(bytes))
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("Validated ASCII")
    }
}
