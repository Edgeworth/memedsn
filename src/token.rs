use derive_more::Display;
use strum::{Display as EnumDisplay, EnumString};

#[must_use]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, EnumString, EnumDisplay)]
#[strum(serialize_all = "snake_case")]
pub enum Tok {
    Area,
    Attach,
    Back,
    Both,
    Boundary,
    Circle,
    Circuit,
    Class,
    Clearance,
    Cm,
    Component,
    Connect,
    DefaultSmd,
    Front,
    Gate,
    Image,
    Inch,
    Jumper,
    Keepout,
    Layer,
    Library,
    Literal,
    LockType,
    #[strum(serialize = "(")]
    Lparen,
    Mil,
    Mixed,
    Mm,
    Net,
    Network,
    Off,
    On,
    Outline,
    Padstack,
    Parser,
    Path,
    Pcb,
    Pin,
    Pins,
    Place,
    Placement,
    Plane,
    Pn,
    Polygon,
    Position,
    Power,
    Property,
    Qarc,
    Rect,
    Reduced,
    Resolution,
    Rotate,
    #[strum(serialize = ")")]
    Rparen,
    Rule,
    Shape,
    Signal,
    Smd,
    SmdSmd,
    Structure,
    Testpoint,
    Type,
    Um,
    Unit,
    UseVia,
    ViaKeepout,
    Via,
    Width,
    Window,
    WireKeepout,
    Wire,
    Wiring,
}

#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[display("Token({tok}:{s})")]
pub struct Token {
    pub tok: Tok,
    pub s: String,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use eyre::Result;

    use super::*;

    #[test]
    fn test_tok_from_str_case_sensitive() -> Result<()> {
        // Tok::from_str is case-sensitive and expects snake_case
        // The lexer handles case-insensitivity by converting to lowercase first
        assert_eq!(Tok::from_str("area")?, Tok::Area);
        assert_eq!(Tok::from_str("pcb")?, Tok::Pcb);
        assert_eq!(Tok::from_str("net")?, Tok::Net);
        // Mixed case should fail
        assert!(Tok::from_str("AREA").is_err());
        assert!(Tok::from_str("Area").is_err());
        Ok(())
    }

    #[test]
    fn test_tok_from_str_invalid() {
        assert!(Tok::from_str("invalid_keyword").is_err());
        assert!(Tok::from_str("notakeyword").is_err());
        assert!(Tok::from_str("").is_err());
    }

    #[test]
    fn test_token_display() {
        let token = Token { tok: Tok::Area, s: "area".to_string() };
        assert_eq!(token.to_string(), "Token(area:area)");

        let token2 = Token { tok: Tok::Literal, s: "my_custom_id".to_string() };
        assert_eq!(token2.to_string(), "Token(literal:my_custom_id)");
    }
}
