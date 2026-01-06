use eyre::Result;
use memedsn::lexer::Lexer;
use memedsn::parser::Parser;
use memedsn::types::DsnPcb;

fn parse_dsn(data: &str) -> Result<DsnPcb> {
    let tokens = Lexer::new(data)?.lex()?;
    Parser::new(&tokens).parse()
}

#[test]
fn complex_pcb_smoke() -> Result<()> {
    let data = r"
        (pcb test_board
            (resolution mm 1000)
            (library
                (padstack pad1 (attach on))
                (image img1 (pin pad1 1 0 0)))
            (network
                (net GND (pins R1-1 R2-2))
                (class power GND VCC))
            (placement
                (component img1 (place R1 10.0 20.0 front 0.0)))
            (structure
                (layer Top (type signal))
                (via via1))
        )
    ";
    let pcb = parse_dsn(data)?;
    assert_eq!(pcb.pcb_id, "test_board");
    assert_eq!(pcb.library.padstacks.len(), 1);
    assert_eq!(pcb.network.nets.len(), 1);
    assert_eq!(pcb.placement.components.len(), 1);
    assert_eq!(pcb.structure.layers.len(), 1);
    Ok(())
}

#[test]
fn parser_directive_with_quotes_and_spaces() -> Result<()> {
    let data = r#"
        (pcb directive_test
            (parser
                (string_quote ")
                (space_in_quoted_tokens on)
            )
            (placement
                (component RES
                    (place "R 1" 10 20 front 0)
                )
            )
        )
    "#;

    let pcb = parse_dsn(data)?;

    assert_eq!(pcb.placement.components[0].refs[0].component_id, "R 1");
    Ok(())
}

#[test]
fn parser_directive_space_in_quoted_tokens_off_without_quotes() -> Result<()> {
    let data = r"
        (pcb directive_test
            (parser
                (space_in_quoted_tokens off)
            )
            (placement
                (component RES
                    (place R1 10 20 front 0)
                )
            )
        )
    ";

    let pcb = parse_dsn(data)?;
    assert_eq!(pcb.placement.components[0].refs[0].component_id, "R1");
    Ok(())
}
