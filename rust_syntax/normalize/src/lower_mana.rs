// Exhaustive conversion of the parser mana vocabulary into stable IR data.
fn mana_symbol(value: &source::ManaSymbol) -> Result<ManaSymbol> {
    use source::ManaSymbol as S;
    Ok(match value {
        S::ManaCostGeneric(n) => ManaSymbol::Generic(
            (*n).try_into()
                .map_err(|_| invalid("negative generic mana cost"))?,
        ),
        S::ManaCostW => ManaSymbol::Color(ManaColor::White),
        S::ManaCostU => ManaSymbol::Color(ManaColor::Blue),
        S::ManaCostB => ManaSymbol::Color(ManaColor::Black),
        S::ManaCostR => ManaSymbol::Color(ManaColor::Red),
        S::ManaCostG => ManaSymbol::Color(ManaColor::Green),
        S::ManaCostC => ManaSymbol::Color(ManaColor::Colorless),
        S::ManaCostS => ManaSymbol::Snow,
        S::ManaCostWP => ManaSymbol::Phyrexian(vec![ManaColor::White]),
        S::ManaCostUP => ManaSymbol::Phyrexian(vec![ManaColor::Blue]),
        S::ManaCostBP => ManaSymbol::Phyrexian(vec![ManaColor::Black]),
        S::ManaCostRP => ManaSymbol::Phyrexian(vec![ManaColor::Red]),
        S::ManaCostGP => ManaSymbol::Phyrexian(vec![ManaColor::Green]),
        S::ManaCost2U => ManaSymbol::MonocoloredHybrid(ManaColor::Blue),
        S::ManaCost2B => ManaSymbol::MonocoloredHybrid(ManaColor::Black),
        S::ManaCost2R => ManaSymbol::MonocoloredHybrid(ManaColor::Red),
        S::ManaCost2G => ManaSymbol::MonocoloredHybrid(ManaColor::Green),
        S::ManaCostWU => ManaSymbol::Hybrid(ManaColor::White, ManaColor::Blue),
        S::ManaCostUB => ManaSymbol::Hybrid(ManaColor::Blue, ManaColor::Black),
        S::ManaCostBR => ManaSymbol::Hybrid(ManaColor::Black, ManaColor::Red),
        S::ManaCostRG => ManaSymbol::Hybrid(ManaColor::Red, ManaColor::Green),
        S::ManaCostGW => ManaSymbol::Hybrid(ManaColor::Green, ManaColor::White),
        S::ManaCostWB => ManaSymbol::Hybrid(ManaColor::White, ManaColor::Black),
        S::ManaCostUR => ManaSymbol::Hybrid(ManaColor::Blue, ManaColor::Red),
        S::ManaCostBG => ManaSymbol::Hybrid(ManaColor::Black, ManaColor::Green),
        S::ManaCostRW => ManaSymbol::Hybrid(ManaColor::Red, ManaColor::White),
        S::ManaCostGU => ManaSymbol::Hybrid(ManaColor::Green, ManaColor::Blue),
    })
}

fn mana_symbol_x(value: &source::ManaSymbolX) -> Result<ManaSymbol> {
    use source::ManaSymbolX as S;
    Ok(match value {
        S::ManaCostGeneric(n) => ManaSymbol::Generic(
            (*n).try_into()
                .map_err(|_| invalid("negative generic mana cost"))?,
        ),
        S::ManaCostW => ManaSymbol::Color(ManaColor::White),
        S::ManaCostU => ManaSymbol::Color(ManaColor::Blue),
        S::ManaCostB => ManaSymbol::Color(ManaColor::Black),
        S::ManaCostR => ManaSymbol::Color(ManaColor::Red),
        S::ManaCostG => ManaSymbol::Color(ManaColor::Green),
        S::ManaCostC => ManaSymbol::Color(ManaColor::Colorless),
        S::ManaCostS => ManaSymbol::Snow,
        S::ManaCostWP => ManaSymbol::Phyrexian(vec![ManaColor::White]),
        S::ManaCostUP => ManaSymbol::Phyrexian(vec![ManaColor::Blue]),
        S::ManaCostBP => ManaSymbol::Phyrexian(vec![ManaColor::Black]),
        S::ManaCostRP => ManaSymbol::Phyrexian(vec![ManaColor::Red]),
        S::ManaCostGP => ManaSymbol::Phyrexian(vec![ManaColor::Green]),
        S::ManaCostRWP => ManaSymbol::Phyrexian(vec![ManaColor::Red, ManaColor::White]),
        S::ManaCostRGP => ManaSymbol::Phyrexian(vec![ManaColor::Red, ManaColor::Green]),
        S::ManaCostGWP => ManaSymbol::Phyrexian(vec![ManaColor::Green, ManaColor::White]),
        S::ManaCostGUP => ManaSymbol::Phyrexian(vec![ManaColor::Green, ManaColor::Blue]),
        S::ManaCost2W => ManaSymbol::MonocoloredHybrid(ManaColor::White),
        S::ManaCost2U => ManaSymbol::MonocoloredHybrid(ManaColor::Blue),
        S::ManaCost2B => ManaSymbol::MonocoloredHybrid(ManaColor::Black),
        S::ManaCost2R => ManaSymbol::MonocoloredHybrid(ManaColor::Red),
        S::ManaCost2G => ManaSymbol::MonocoloredHybrid(ManaColor::Green),
        S::ManaCostCW => ManaSymbol::Hybrid(ManaColor::Colorless, ManaColor::White),
        S::ManaCostCU => ManaSymbol::Hybrid(ManaColor::Colorless, ManaColor::Blue),
        S::ManaCostCB => ManaSymbol::Hybrid(ManaColor::Colorless, ManaColor::Black),
        S::ManaCostCR => ManaSymbol::Hybrid(ManaColor::Colorless, ManaColor::Red),
        S::ManaCostCG => ManaSymbol::Hybrid(ManaColor::Colorless, ManaColor::Green),
        S::ManaCostWU => ManaSymbol::Hybrid(ManaColor::White, ManaColor::Blue),
        S::ManaCostUB => ManaSymbol::Hybrid(ManaColor::Blue, ManaColor::Black),
        S::ManaCostBR => ManaSymbol::Hybrid(ManaColor::Black, ManaColor::Red),
        S::ManaCostRG => ManaSymbol::Hybrid(ManaColor::Red, ManaColor::Green),
        S::ManaCostGW => ManaSymbol::Hybrid(ManaColor::Green, ManaColor::White),
        S::ManaCostWB => ManaSymbol::Hybrid(ManaColor::White, ManaColor::Black),
        S::ManaCostUR => ManaSymbol::Hybrid(ManaColor::Blue, ManaColor::Red),
        S::ManaCostBG => ManaSymbol::Hybrid(ManaColor::Black, ManaColor::Green),
        S::ManaCostRW => ManaSymbol::Hybrid(ManaColor::Red, ManaColor::White),
        S::ManaCostGU => ManaSymbol::Hybrid(ManaColor::Green, ManaColor::Blue),
        S::ManaCostX => ManaSymbol::X,
    })
}
