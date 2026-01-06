use std::str::FromStr;

use eyre::{Result, eyre};
use memegeom::primitive::point::Pt;
use memegeom::primitive::{Rt, pt};

use crate::token::{Tok, Token};
use crate::types::{
    DsnCircle, DsnCircuit, DsnClass, DsnClearance, DsnClearanceType, DsnComponent,
    DsnDimensionUnit, DsnImage, DsnKeepout, DsnKeepoutType, DsnLayer, DsnLayerType, DsnLibrary,
    DsnLockType, DsnNet, DsnNetwork, DsnPadstack, DsnPadstackShape, DsnPath, DsnPcb, DsnPin,
    DsnPinRef, DsnPlacement, DsnPlacementRef, DsnPlane, DsnPolygon, DsnQArc, DsnRect,
    DsnResolution, DsnRule, DsnShape, DsnSide, DsnStructure, DsnVia, DsnWindow, DsnWire, DsnWiring,
};

#[must_use]
#[derive(Debug, Clone, PartialEq)]
pub struct Parser {
    toks: Vec<Token>,
    idx: usize,
    pcb: DsnPcb,
}

impl Parser {
    pub fn new(toks: &[Token]) -> Self {
        Self { toks: toks.to_vec(), idx: 0, pcb: DsnPcb::default() }
    }

    pub fn parse(mut self) -> Result<DsnPcb> {
        self.pcb()?;
        Ok(self.pcb)
    }

    fn peek(&self, ahead: usize) -> Result<&Token> {
        if self.idx + ahead < self.toks.len() {
            Ok(&self.toks[self.idx + ahead])
        } else {
            Err(eyre!("unexpected EOF"))
        }
    }

    fn next(&mut self) -> Result<&Token> {
        if self.idx < self.toks.len() {
            self.idx += 1;
            Ok(&self.toks[self.idx - 1])
        } else {
            Err(eyre!("unexpected EOF"))
        }
    }

    fn expect(&mut self, t: Tok) -> Result<&Token> {
        match self.next()? {
            x if x.tok == t => Ok(x),
            x => Err(eyre!("unexpected token {}", x)),
        }
    }

    fn literal(&mut self) -> Result<&str> {
        Ok(&self.next()?.s)
    }

    fn ignore(&mut self) -> Result<()> {
        let inside_expr = self.peek(0)?.tok != Tok::Lparen;
        loop {
            let t = self.next()?;
            if t.tok == Tok::Rparen {
                break;
            }
            if t.tok == Tok::Lparen {
                self.ignore()?;
                // Handle the case of being called at the start of an expression.
                if !inside_expr {
                    break;
                }
            }
        }
        Ok(())
    }

    fn pcb(&mut self) -> Result<()> {
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Pcb)?;
        self.pcb.pcb_id = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Library => self.pcb.library = self.library()?,
                Tok::Network => self.pcb.network = self.network()?,
                Tok::Parser => self.ignore()?, // Handled during lexing.
                Tok::Placement => self.pcb.placement = self.placement()?,
                Tok::Resolution => self.pcb.resolution = self.resolution()?,
                Tok::Structure => self.pcb.structure = self.structure()?,
                Tok::Unit => self.pcb.unit.dimension = self.unit()?,
                Tok::Wiring => self.pcb.wiring = self.wiring()?,
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(())
    }

    fn library(&mut self) -> Result<DsnLibrary> {
        let mut v = DsnLibrary::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Library)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Image => v.images.push(self.image()?),
                Tok::Padstack => v.padstacks.push(self.padstack()?),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn network(&mut self) -> Result<DsnNetwork> {
        let mut v = DsnNetwork::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Network)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Class => v.classes.push(self.class()?),
                Tok::Net => v.nets.push(self.net()?),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn placement(&mut self) -> Result<DsnPlacement> {
        let mut v = DsnPlacement::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Placement)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Component => v.components.push(self.component()?),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn resolution(&mut self) -> Result<DsnResolution> {
        let mut v = DsnResolution::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Resolution)?;
        v.dimension = self.dimension()?;
        v.amount = self.integer()?;
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn structure(&mut self) -> Result<DsnStructure> {
        let mut v = DsnStructure::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Structure)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Boundary => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Boundary)?;
                    v.boundaries.push(self.shape()?);
                    self.expect(Tok::Rparen)?;
                }
                Tok::Keepout | Tok::ViaKeepout | Tok::WireKeepout => {
                    v.keepouts.push(self.keepout()?);
                }
                Tok::Layer => v.layers.push(self.layer()?),
                Tok::Plane => v.planes.push(self.plane()?),
                Tok::Rule => v.rules.extend(self.rule()?),
                Tok::Via => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Via)?;
                    while self.peek(0)?.tok != Tok::Rparen {
                        v.vias.push(self.literal()?.to_string());
                    }
                    self.expect(Tok::Rparen)?;
                }
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn wiring(&mut self) -> Result<DsnWiring> {
        let mut v = DsnWiring::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Wiring)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Wire => v.wires.push(self.wire()?),
                Tok::Via => v.vias.push(self.via()?),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn via(&mut self) -> Result<DsnVia> {
        let v = DsnVia::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Via)?;
        // TODO: Finish.
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn wire(&mut self) -> Result<DsnWire> {
        let v = DsnWire::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Wire)?;
        // TODO: Finish.
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn layer(&mut self) -> Result<DsnLayer> {
        let mut v = DsnLayer::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Layer)?;
        v.layer_name = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Type => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Type)?;
                    match self.next()?.tok {
                        Tok::Jumper => v.layer_type = DsnLayerType::Jumper,
                        Tok::Mixed => v.layer_type = DsnLayerType::Mixed,
                        Tok::Power => v.layer_type = DsnLayerType::Power,
                        Tok::Signal => v.layer_type = DsnLayerType::Signal,
                        _ => return Err(eyre!("unrecognised layer type")),
                    }
                    self.expect(Tok::Rparen)?;
                }
                Tok::Property => self.ignore()?, // Ignore user properties.
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn plane(&mut self) -> Result<DsnPlane> {
        let v = DsnPlane::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Plane)?;
        // TODO: Finish.
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn component(&mut self) -> Result<DsnComponent> {
        let mut v = DsnComponent::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Component)?;
        v.image_id = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Place => v.refs.push(self.placement_ref()?),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn placement_ref(&mut self) -> Result<DsnPlacementRef> {
        let mut v = DsnPlacementRef::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Place)?;
        v.component_id = self.literal()?.to_string();
        v.p = self.vertex()?; // Assume we have vertex information.
        v.side = self.side()?;
        v.rotation = self.number()?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::LockType => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::LockType)?;
                    match self.next()?.tok {
                        Tok::Gate => v.lock_type = DsnLockType::Gate,
                        Tok::Position => v.lock_type = DsnLockType::Position,
                        _ => return Err(eyre!("unrecognised lock type")),
                    }
                    self.expect(Tok::Rparen)?;
                }
                Tok::Pn => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Pn)?;
                    v.part_number = self.literal()?.to_string();
                    self.expect(Tok::Rparen)?;
                }
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn image(&mut self) -> Result<DsnImage> {
        let mut v = DsnImage::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Image)?;
        v.image_id = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Outline => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Outline)?;
                    v.outlines.push(self.shape()?);
                    self.expect(Tok::Rparen)?;
                }
                Tok::Pin => v.pins.push(self.pin()?),
                Tok::Keepout | Tok::ViaKeepout | Tok::WireKeepout => {
                    v.keepouts.push(self.keepout()?);
                }
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn keepout(&mut self) -> Result<DsnKeepout> {
        let mut v = DsnKeepout::default();
        self.expect(Tok::Lparen)?;
        v.keepout_type = match self.next()?.tok {
            Tok::Keepout => DsnKeepoutType::Keepout,
            Tok::ViaKeepout => DsnKeepoutType::ViaKeepout,
            Tok::WireKeepout => DsnKeepoutType::WireKeepout,
            _ => return Err(eyre!("unrecognised keepout type")),
        };
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Rect | Tok::Circle | Tok::Polygon | Tok::Path | Tok::Qarc => {
                    v.shape = self.shape()?;
                }
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn pin(&mut self) -> Result<DsnPin> {
        let mut v = DsnPin::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Pin)?;
        v.padstack_id = self.literal()?.to_string();
        if self.peek(0)?.tok == Tok::Lparen {
            // Rotation.
            self.expect(Tok::Lparen)?;
            self.expect(Tok::Rotate)?;
            v.rotation = self.number()?;
            self.expect(Tok::Rparen)?;
        }
        v.pin_id = self.literal()?.to_string();
        v.p = self.vertex()?;
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn padstack(&mut self) -> Result<DsnPadstack> {
        let mut v = DsnPadstack::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Padstack)?;
        v.padstack_id = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Attach => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Attach)?;
                    v.attach = self.onoff()?;
                    self.expect(Tok::Rparen)?;
                }
                Tok::Shape => v.shapes.push(self.padstack_shape()?),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn padstack_shape(&mut self) -> Result<DsnPadstackShape> {
        let mut v = DsnPadstackShape::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Shape)?;
        v.shape = self.shape()?;
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn clearance(&mut self) -> Result<DsnClearance> {
        let mut v = DsnClearance::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Clearance)?;
        v.amount = self.number()?;

        while self.peek(0)?.tok != Tok::Rparen {
            self.expect(Tok::Lparen)?;
            self.expect(Tok::Type)?;
            v.types.push(match self.next()?.tok {
                Tok::DefaultSmd => DsnClearanceType::DefaultSmd,
                Tok::SmdSmd => DsnClearanceType::SmdSmd,
                _ => return Err(eyre!("unrecognised clearance type")),
            });
            self.expect(Tok::Rparen)?;
        }

        // If no type is specified, assume it applies to everything.
        if v.types.is_empty() {
            v.types.push(DsnClearanceType::All);
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    #[allow(dead_code)]
    fn window(&mut self) -> Result<DsnWindow> {
        let v = DsnWindow::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Window)?;
        // TODO: Finish.
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn shape(&mut self) -> Result<DsnShape> {
        match self.peek(1)?.tok {
            Tok::Circle => Ok(DsnShape::Circle(self.circle()?)),
            Tok::Path => Ok(DsnShape::Path(self.path()?)),
            Tok::Polygon => Ok(DsnShape::Polygon(self.polygon()?)),
            Tok::Qarc => Ok(DsnShape::QArc(self.qarc()?)),
            Tok::Rect => Ok(DsnShape::Rect(self.rect()?)),
            _ => Err(eyre!("unrecognised shape type")),
        }
    }

    fn rect(&mut self) -> Result<DsnRect> {
        let mut v = DsnRect::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Rect)?;
        v.layer_id = self.literal()?.to_string();
        let a = self.vertex()?;
        let b = self.vertex()?;
        v.rect = Rt::enclosing(a, b); // Opposite points but can be in either order.
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn circle(&mut self) -> Result<DsnCircle> {
        let mut v = DsnCircle::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Circle)?;
        v.layer_id = self.literal()?.to_string();
        v.diameter = self.number()?;
        if self.peek(0)?.tok != Tok::Rparen {
            v.p = self.vertex()?;
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn polygon(&mut self) -> Result<DsnPolygon> {
        let mut v = DsnPolygon::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Polygon)?;
        v.layer_id = self.literal()?.to_string();
        v.aperture_width = self.number()?;
        while self.peek(0)?.tok != Tok::Rparen {
            v.pts.push(self.vertex()?);
        }
        self.expect(Tok::Rparen)?;
        if v.pts.len() < 3 {
            return Err(eyre!("polygon must have at least three points"));
        }
        Ok(v)
    }

    fn path(&mut self) -> Result<DsnPath> {
        let mut v = DsnPath::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Path)?;
        v.layer_id = self.literal()?.to_string();
        v.aperture_width = self.number()?;
        while self.peek(0)?.tok != Tok::Rparen {
            v.pts.push(self.vertex()?);
        }
        self.expect(Tok::Rparen)?;
        if v.pts.len() < 2 {
            return Err(eyre!("path must have at least two points"));
        }
        Ok(v)
    }

    fn qarc(&mut self) -> Result<DsnQArc> {
        let mut v = DsnQArc::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Qarc)?;
        v.layer_id = self.literal()?.to_string();
        v.aperture_width = self.number()?;
        v.start = self.vertex()?;
        v.end = self.vertex()?;
        v.center = self.vertex()?;
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn class(&mut self) -> Result<DsnClass> {
        let mut v = DsnClass::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Class)?;
        v.class_id = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let pt = self.peek(0)?;
            if pt.tok == Tok::Lparen {
                let t = self.peek(1)?;
                match t.tok {
                    Tok::Circuit => v.circuits.extend(self.circuit()?),
                    Tok::Rule => v.rules.extend(self.rule()?),
                    _ => return Err(eyre!("unrecognised token '{}'", t)),
                }
            } else {
                v.net_ids.push(self.literal()?.to_string());
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn circuit(&mut self) -> Result<Vec<DsnCircuit>> {
        let mut v = Vec::new();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Circuit)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::UseVia => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::UseVia)?;
                    v.push(DsnCircuit::UseVia(self.literal()?.to_string()));
                    self.expect(Tok::Rparen)?;
                }
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn net(&mut self) -> Result<DsnNet> {
        let mut v = DsnNet::default();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Net)?;
        v.net_id = self.literal()?.to_string();
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Pins => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Pins)?;
                    while self.peek(0)?.tok != Tok::Rparen {
                        v.pins.push(self.pin_ref()?);
                    }
                    self.expect(Tok::Rparen)?;
                }
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn rule(&mut self) -> Result<Vec<DsnRule>> {
        let mut v = Vec::new();
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Rule)?;
        while self.peek(0)?.tok != Tok::Rparen {
            let t = self.peek(1)?;
            match t.tok {
                Tok::Width => {
                    self.expect(Tok::Lparen)?;
                    self.expect(Tok::Width)?;
                    let width = self.number()?;
                    v.push(DsnRule::Width(width));
                    self.expect(Tok::Rparen)?;
                }
                Tok::Clearance => v.push(DsnRule::Clearance(self.clearance()?)),
                _ => return Err(eyre!("unrecognised token '{}'", t)),
            }
        }
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn vertex(&mut self) -> Result<Pt> {
        Ok(pt(self.number()?, self.number()?))
    }

    fn unit(&mut self) -> Result<DsnDimensionUnit> {
        self.expect(Tok::Lparen)?;
        self.expect(Tok::Unit)?;
        let v = self.dimension()?;
        self.expect(Tok::Rparen)?;
        Ok(v)
    }

    fn pin_ref(&mut self) -> Result<DsnPinRef> {
        let p = self.literal()?;
        let (a, b) = p.rsplit_once('-').ok_or_else(|| eyre!("invalid pin reference {}", p))?;
        Ok(DsnPinRef { component_id: a.to_owned(), pin_id: b.to_owned() })
    }

    fn onoff(&mut self) -> Result<bool> {
        match self.next()?.tok {
            Tok::Off => Ok(false),
            Tok::On => Ok(true),
            _ => Err(eyre!("expected off or on")),
        }
    }

    fn side(&mut self) -> Result<DsnSide> {
        match self.next()?.tok {
            Tok::Back => Ok(DsnSide::Back),
            Tok::Both => Ok(DsnSide::Both),
            Tok::Front => Ok(DsnSide::Front),
            _ => Err(eyre!("unrecognised side type")),
        }
    }

    fn dimension(&mut self) -> Result<DsnDimensionUnit> {
        Ok(match self.next()?.tok {
            Tok::Inch => DsnDimensionUnit::Inch,
            Tok::Mil => DsnDimensionUnit::Mil,
            Tok::Cm => DsnDimensionUnit::Cm,
            Tok::Mm => DsnDimensionUnit::Mm,
            Tok::Um => DsnDimensionUnit::Um,
            _ => return Err(eyre!("unknown dimension unit")),
        })
    }

    fn number(&mut self) -> Result<f64> {
        // TODO: Handle fractions.
        Ok(f64::from_str(self.literal()?)?)
    }

    fn integer(&mut self) -> Result<i32> {
        // TODO: Handle fractions.
        Ok(i32::from_str(self.literal()?)?)
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_dsn(data: &str) -> Result<DsnPcb> {
        let lexer = Lexer::new(data)?;
        let tokens = lexer.lex()?;
        let parser = Parser::new(&tokens);
        parser.parse()
    }

    #[test]
    fn minimal_pcb() -> Result<()> {
        let data = "(pcb test)";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.pcb_id, "test");
        Ok(())
    }

    #[test]
    fn pcb_with_resolution() -> Result<()> {
        let data = "(pcb test (resolution mm 1000))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.pcb_id, "test");
        assert_eq!(pcb.resolution.dimension, DsnDimensionUnit::Mm);
        assert_eq!(pcb.resolution.amount, 1000);
        Ok(())
    }

    #[test]
    fn pcb_with_unit() -> Result<()> {
        let data = "(pcb test (unit mm))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.unit.dimension, DsnDimensionUnit::Mm);
        Ok(())
    }

    #[test]
    fn library_with_padstack() -> Result<()> {
        let data = "(pcb test (library (padstack pad1 (attach off))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.padstacks.len(), 1);
        assert_eq!(pcb.library.padstacks[0].padstack_id, "pad1");
        assert!(!pcb.library.padstacks[0].attach);
        Ok(())
    }

    #[test]
    fn library_with_padstack_attach_on() -> Result<()> {
        let data = "(pcb test (library (padstack pad1 (attach on))))";
        let pcb = parse_dsn(data)?;
        assert!(pcb.library.padstacks[0].attach);
        Ok(())
    }

    #[test]
    fn library_with_image() -> Result<()> {
        let data = "(pcb test (library (image img1)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.images.len(), 1);
        assert_eq!(pcb.library.images[0].image_id, "img1");
        Ok(())
    }

    #[test]
    fn image_with_pin() -> Result<()> {
        let data = "(pcb test (library (image img1 (pin pad1 1 10.0 20.0))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.images[0].pins.len(), 1);
        assert_eq!(pcb.library.images[0].pins[0].padstack_id, "pad1");
        assert_eq!(pcb.library.images[0].pins[0].pin_id, "1");
        Ok(())
    }

    #[test]
    fn image_with_pin_rotation() -> Result<()> {
        let data = "(pcb test (library (image img1 (pin pad1 (rotate 90.0) 1 10.0 20.0))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.images[0].pins[0].rotation, 90.0);
        Ok(())
    }

    #[test]
    fn network_with_net() -> Result<()> {
        let data = "(pcb test (network (net net1)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.nets.len(), 1);
        assert_eq!(pcb.network.nets[0].net_id, "net1");
        Ok(())
    }

    #[test]
    fn net_with_pins() -> Result<()> {
        let data = "(pcb test (network (net net1 (pins R1-1 R2-2))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.nets[0].pins.len(), 2);
        assert_eq!(pcb.network.nets[0].pins[0].component_id, "R1");
        assert_eq!(pcb.network.nets[0].pins[0].pin_id, "1");
        assert_eq!(pcb.network.nets[0].pins[1].component_id, "R2");
        assert_eq!(pcb.network.nets[0].pins[1].pin_id, "2");
        Ok(())
    }

    #[test]
    fn network_with_class() -> Result<()> {
        let data = "(pcb test (network (class power GND VCC)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.classes.len(), 1);
        assert_eq!(pcb.network.classes[0].class_id, "power");
        assert_eq!(pcb.network.classes[0].net_ids.len(), 2);
        assert_eq!(pcb.network.classes[0].net_ids[0], "GND");
        assert_eq!(pcb.network.classes[0].net_ids[1], "VCC");
        Ok(())
    }

    #[test]
    fn class_with_circuit() -> Result<()> {
        let data = "(pcb test (network (class signal (circuit (use_via via1)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.classes[0].circuits.len(), 1);
        match &pcb.network.classes[0].circuits[0] {
            DsnCircuit::UseVia(s) => assert_eq!(s, "via1"),
        }
        Ok(())
    }

    #[test]
    fn class_with_rule_width() -> Result<()> {
        let data = "(pcb test (network (class signal (rule (width 0.5)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.classes[0].rules.len(), 1);
        match &pcb.network.classes[0].rules[0] {
            DsnRule::Width(w) => assert_eq!(*w, 0.5),
            DsnRule::Clearance(_) => panic!("Expected width rule"),
        }
        Ok(())
    }

    #[test]
    fn class_with_rule_clearance() -> Result<()> {
        let data = "(pcb test (network (class signal (rule (clearance 0.3)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.classes[0].rules.len(), 1);
        match &pcb.network.classes[0].rules[0] {
            DsnRule::Clearance(c) => {
                assert_eq!(c.amount, 0.3);
                assert_eq!(c.types.len(), 1);
                match c.types[0] {
                    DsnClearanceType::All => (),
                    _ => panic!("Expected All clearance type"),
                }
            }
            DsnRule::Width(_) => panic!("Expected clearance rule"),
        }
        Ok(())
    }

    #[test]
    fn clearance_with_type() -> Result<()> {
        let data = "(pcb test (network (class signal (rule (clearance 0.3 (type smd_smd))))))";
        let pcb = parse_dsn(data)?;
        match &pcb.network.classes[0].rules[0] {
            DsnRule::Clearance(c) => {
                assert_eq!(c.types.len(), 1);
                match c.types[0] {
                    DsnClearanceType::SmdSmd => (),
                    _ => panic!("Expected SmdSmd clearance type"),
                }
            }
            DsnRule::Width(_) => panic!("Expected clearance rule"),
        }
        Ok(())
    }

    #[test]
    fn placement_with_component() -> Result<()> {
        let data = "(pcb test (placement (component img1)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.placement.components.len(), 1);
        assert_eq!(pcb.placement.components[0].image_id, "img1");
        Ok(())
    }

    #[test]
    fn component_with_place() -> Result<()> {
        let data = "(pcb test (placement (component img1 (place R1 10.0 20.0 front 0.0))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.placement.components[0].refs.len(), 1);
        assert_eq!(pcb.placement.components[0].refs[0].component_id, "R1");
        assert_eq!(pcb.placement.components[0].refs[0].side, DsnSide::Front);
        assert_eq!(pcb.placement.components[0].refs[0].rotation, 0.0);
        Ok(())
    }

    #[test]
    fn place_with_lock_type() -> Result<()> {
        let data = "(pcb test (placement (component img1 (place R1 10.0 20.0 front 0.0 (lock_type position)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.placement.components[0].refs[0].lock_type, DsnLockType::Position);
        Ok(())
    }

    #[test]
    fn place_with_part_number() -> Result<()> {
        let data =
            "(pcb test (placement (component img1 (place R1 10.0 20.0 front 0.0 (pn 1234)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.placement.components[0].refs[0].part_number, "1234");
        Ok(())
    }

    #[test]
    fn layer_types() -> Result<()> {
        let data = "(pcb test (structure (layer L1 (type signal)) (layer L2 (type power)) (layer L3 (type mixed)) (layer L4 (type jumper))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.layers.len(), 4);
        assert_eq!(pcb.structure.layers[0].layer_name, "L1");
        assert_eq!(pcb.structure.layers[0].layer_type, DsnLayerType::Signal);
        assert_eq!(pcb.structure.layers[1].layer_name, "L2");
        assert_eq!(pcb.structure.layers[1].layer_type, DsnLayerType::Power);
        assert_eq!(pcb.structure.layers[2].layer_name, "L3");
        assert_eq!(pcb.structure.layers[2].layer_type, DsnLayerType::Mixed);
        assert_eq!(pcb.structure.layers[3].layer_name, "L4");
        assert_eq!(pcb.structure.layers[3].layer_type, DsnLayerType::Jumper);
        Ok(())
    }

    #[test]
    fn structure_with_via() -> Result<()> {
        let data = "(pcb test (structure (via via1 via2)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.vias.len(), 2);
        assert_eq!(pcb.structure.vias[0], "via1");
        assert_eq!(pcb.structure.vias[1], "via2");
        Ok(())
    }

    #[test]
    fn rect_shape() -> Result<()> {
        let data = "(pcb test (structure (boundary (rect Top 0 0 100 100))))";
        let pcb = parse_dsn(data)?;
        match &pcb.structure.boundaries[0] {
            DsnShape::Rect(r) => {
                assert_eq!(r.layer_id, "Top");
            }
            _ => panic!("Expected rect shape"),
        }
        Ok(())
    }

    #[test]
    fn circle_shape() -> Result<()> {
        let data = "(pcb test (library (padstack pad1 (shape (circle Top 10.0)))))";
        let pcb = parse_dsn(data)?;
        match &pcb.library.padstacks[0].shapes[0].shape {
            DsnShape::Circle(c) => {
                assert_eq!(c.layer_id, "Top");
                assert_eq!(c.diameter, 10.0);
            }
            _ => panic!("Expected circle shape"),
        }
        Ok(())
    }

    #[test]
    fn circle_shape_with_position() -> Result<()> {
        let data = "(pcb test (library (padstack pad1 (shape (circle Top 10.0 5.0 5.0)))))";
        let pcb = parse_dsn(data)?;
        match &pcb.library.padstacks[0].shapes[0].shape {
            DsnShape::Circle(c) => {
                assert_eq!(c.p.x, 5.0);
                assert_eq!(c.p.y, 5.0);
            }
            _ => panic!("Expected circle shape"),
        }
        Ok(())
    }

    #[test]
    fn polygon_shape() -> Result<()> {
        let data = "(pcb test (structure (boundary (polygon Top 1.0 0 0 10 0 10 10 0 10))))";
        let pcb = parse_dsn(data)?;
        match &pcb.structure.boundaries[0] {
            DsnShape::Polygon(p) => {
                assert_eq!(p.layer_id, "Top");
                assert_eq!(p.aperture_width, 1.0);
                assert_eq!(p.pts.len(), 4);
            }
            _ => panic!("Expected polygon shape"),
        }
        Ok(())
    }

    #[test]
    fn path_shape() -> Result<()> {
        let data = "(pcb test (structure (boundary (path Top 1.0 0 0 10 10 20 20))))";
        let pcb = parse_dsn(data)?;
        match &pcb.structure.boundaries[0] {
            DsnShape::Path(p) => {
                assert_eq!(p.layer_id, "Top");
                assert_eq!(p.aperture_width, 1.0);
                assert_eq!(p.pts.len(), 3);
            }
            _ => panic!("Expected path shape"),
        }
        Ok(())
    }

    #[test]
    fn qarc_shape() -> Result<()> {
        let data = "(pcb test (structure (boundary (qarc Top 1.0 0 0 10 10 5 5))))";
        let pcb = parse_dsn(data)?;
        match &pcb.structure.boundaries[0] {
            DsnShape::QArc(q) => {
                assert_eq!(q.layer_id, "Top");
                assert_eq!(q.aperture_width, 1.0);
                assert_eq!(q.start.x, 0.0);
                assert_eq!(q.end.x, 10.0);
                assert_eq!(q.center.x, 5.0);
            }
            _ => panic!("Expected qarc shape"),
        }
        Ok(())
    }

    #[test]
    fn keepout() -> Result<()> {
        let data = "(pcb test (structure (keepout (rect Top 0 0 10 10))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.keepouts.len(), 1);
        assert_eq!(pcb.structure.keepouts[0].keepout_type, DsnKeepoutType::Keepout);
        Ok(())
    }

    #[test]
    fn via_keepout() -> Result<()> {
        let data = "(pcb test (structure (via_keepout (rect Top 0 0 10 10))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.keepouts[0].keepout_type, DsnKeepoutType::ViaKeepout);
        Ok(())
    }

    #[test]
    fn wire_keepout() -> Result<()> {
        let data = "(pcb test (structure (wire_keepout (rect Top 0 0 10 10))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.keepouts[0].keepout_type, DsnKeepoutType::WireKeepout);
        Ok(())
    }

    #[test]
    fn image_with_keepout() -> Result<()> {
        let data = "(pcb test (library (image img1 (keepout (rect Top 0 0 10 10)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.images[0].keepouts.len(), 1);
        Ok(())
    }

    #[test]
    fn image_with_outline() -> Result<()> {
        let data = "(pcb test (library (image img1 (outline (rect Top 0 0 10 10)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.images[0].outlines.len(), 1);
        Ok(())
    }

    #[test]
    fn negative_numbers() -> Result<()> {
        let data = "(pcb test (placement (component img1 (place R1 -10.5 -20.3 front 0.0))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.placement.components[0].refs[0].p.x, -10.5);
        assert_eq!(pcb.placement.components[0].refs[0].p.y, -20.3);
        Ok(())
    }

    #[test]
    fn decimal_numbers() -> Result<()> {
        let data = "(pcb test (resolution mm 1000) (network (class signal (rule (width 0.254)))))";
        let pcb = parse_dsn(data)?;
        match &pcb.network.classes[0].rules[0] {
            DsnRule::Width(w) => assert_eq!(*w, 0.254),
            DsnRule::Clearance(_) => panic!("Expected width rule"),
        }
        Ok(())
    }

    #[test]
    fn multiple_boundaries() -> Result<()> {
        let data =
            "(pcb test (structure (boundary (rect Top 0 0 100 100)) (boundary (circle Top 10))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.boundaries.len(), 2);
        Ok(())
    }

    #[test]
    fn multiple_rules() -> Result<()> {
        let data = "(pcb test (structure (rule (width 0.5) (clearance 0.3))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.rules.len(), 2);
        Ok(())
    }

    #[test]
    fn padstack_with_multiple_shapes() -> Result<()> {
        let data = r"
            (pcb test (library (padstack pad1
                (shape (circle Top 10.0))
                (shape (circle Bottom 8.0))
            )))
        ";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.padstacks[0].shapes.len(), 2);
        Ok(())
    }

    #[test]
    fn error_empty_polygon() {
        let data = "(pcb test (structure (boundary (polygon Top 1.0))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_polygon_insufficient_points() {
        let data = "(pcb test (structure (boundary (polygon Top 1.0 0 0 10 10))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_empty_path() {
        let data = "(pcb test (structure (boundary (path Top 1.0))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_path_insufficient_points() {
        let data = "(pcb test (structure (boundary (path Top 1.0 0 0))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_on_unexpected_token() {
        let data = "(pcb test (unknown_keyword))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_on_missing_rparen() {
        let data = "(pcb test";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_on_unexpected_eof() {
        let data = "(pcb test (library";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn side_variants() -> Result<()> {
        let data_front = "(pcb test (placement (component img1 (place R1 0 0 front 0))))";
        let data_back = "(pcb test (placement (component img1 (place R1 0 0 back 0))))";
        let data_both = "(pcb test (placement (component img1 (place R1 0 0 both 0))))";

        let pcb_front = parse_dsn(data_front)?;
        let pcb_back = parse_dsn(data_back)?;
        let pcb_both = parse_dsn(data_both)?;

        assert_eq!(pcb_front.placement.components[0].refs[0].side, DsnSide::Front);
        assert_eq!(pcb_back.placement.components[0].refs[0].side, DsnSide::Back);
        assert_eq!(pcb_both.placement.components[0].refs[0].side, DsnSide::Both);
        Ok(())
    }

    #[test]
    fn lock_type_gate() -> Result<()> {
        let data =
            "(pcb test (placement (component img1 (place R1 0 0 front 0 (lock_type gate)))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.placement.components[0].refs[0].lock_type, DsnLockType::Gate);
        Ok(())
    }

    #[test]
    fn all_dimension_units() -> Result<()> {
        let data_inch = "(pcb test (resolution inch 1000))";
        let data_mil = "(pcb test (resolution mil 1000))";
        let data_cm = "(pcb test (resolution cm 1000))";
        let data_mm = "(pcb test (resolution mm 1000))";
        let data_um = "(pcb test (resolution um 1000))";

        assert_eq!(parse_dsn(data_inch)?.resolution.dimension, DsnDimensionUnit::Inch);
        assert_eq!(parse_dsn(data_mil)?.resolution.dimension, DsnDimensionUnit::Mil);
        assert_eq!(parse_dsn(data_cm)?.resolution.dimension, DsnDimensionUnit::Cm);
        assert_eq!(parse_dsn(data_mm)?.resolution.dimension, DsnDimensionUnit::Mm);
        assert_eq!(parse_dsn(data_um)?.resolution.dimension, DsnDimensionUnit::Um);
        Ok(())
    }

    #[test]
    fn parser_directive_ignored() -> Result<()> {
        let data = "(pcb test (parser (host_cad freeroute)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.pcb_id, "test");
        Ok(())
    }

    #[test]
    fn layer_with_property_ignored() -> Result<()> {
        let data = "(pcb test (structure (layer Top (type signal) (property user_value 123))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.structure.layers[0].layer_name, "Top");
        Ok(())
    }

    #[test]
    fn clearance_with_default_smd() -> Result<()> {
        let data = "(pcb test (network (class signal (rule (clearance 0.3 (type default_smd))))))";
        let pcb = parse_dsn(data)?;
        match &pcb.network.classes[0].rules[0] {
            DsnRule::Clearance(c) => {
                assert_eq!(c.types.len(), 1);
                match c.types[0] {
                    DsnClearanceType::DefaultSmd => (),
                    _ => panic!("Expected DefaultSmd clearance type"),
                }
            }
            DsnRule::Width(_) => panic!("Expected clearance rule"),
        }
        Ok(())
    }

    #[test]
    fn clearance_with_multiple_types() -> Result<()> {
        let data = "(pcb test (network (class signal (rule (clearance 0.3 (type default_smd) (type smd_smd))))))";
        let pcb = parse_dsn(data)?;
        match &pcb.network.classes[0].rules[0] {
            DsnRule::Clearance(c) => {
                assert_eq!(c.types.len(), 2);
            }
            DsnRule::Width(_) => panic!("Expected clearance rule"),
        }
        Ok(())
    }

    #[test]
    fn error_invalid_dimension_unit() {
        let data = "(pcb test (resolution invalid 1000))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_invalid_layer_type() {
        let data = "(pcb test (structure (layer Top (type invalid))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_invalid_side() {
        let data = "(pcb test (placement (component img1 (place R1 0 0 invalid 0))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_invalid_number() {
        let data = "(pcb test (resolution mm notanumber))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn error_missing_required_field() {
        let data = "(pcb test (resolution))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn pin_reference_parsing() -> Result<()> {
        let data = "(pcb test (network (net GND (pins R1-1 C2-2 U3-10 R-A-1 U-1-2))))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.nets[0].pins.len(), 5);
        assert_eq!(pcb.network.nets[0].pins[2].component_id, "U3");
        assert_eq!(pcb.network.nets[0].pins[2].pin_id, "10");
        assert_eq!(pcb.network.nets[0].pins[3].component_id, "R-A");
        assert_eq!(pcb.network.nets[0].pins[3].pin_id, "1");
        assert_eq!(pcb.network.nets[0].pins[4].component_id, "U-1");
        assert_eq!(pcb.network.nets[0].pins[4].pin_id, "2");
        Ok(())
    }

    #[test]
    fn error_invalid_pin_reference() {
        let data = "(pcb test (network (net GND (pins R1_invalid))))";
        assert!(parse_dsn(data).is_err());
    }

    #[test]
    fn multiple_images_in_library() -> Result<()> {
        let data = "(pcb test (library (image img1) (image img2) (image img3)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.library.images.len(), 3);
        Ok(())
    }

    #[test]
    fn multiple_nets_in_network() -> Result<()> {
        let data = "(pcb test (network (net GND) (net VCC) (net DATA)))";
        let pcb = parse_dsn(data)?;
        assert_eq!(pcb.network.nets.len(), 3);
        Ok(())
    }

    #[test]
    fn empty_sections() -> Result<()> {
        let data = "(pcb test (library) (network) (placement) (wiring))";
        let pcb = parse_dsn(data)?;
        assert!(pcb.library.images.is_empty());
        assert!(pcb.library.padstacks.is_empty());
        assert!(pcb.network.nets.is_empty());
        assert!(pcb.network.classes.is_empty());
        assert!(pcb.placement.components.is_empty());
        assert!(pcb.wiring.wires.is_empty());
        assert!(pcb.wiring.vias.is_empty());
        Ok(())
    }
}
