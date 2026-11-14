//! Exact finite choices of complete mana bundles. The native recipe owns both
//! actual production and advice; an opaque effect is never given a guessed yield.
use super::*;

fn constant_bundles(mana: &ManaProduction) -> Result<Option<Vec<[u32; 6]>>> {
    let ManaProduction::ChooseOne(options) = mana else {
        return Ok(None);
    };
    let mut pools = Vec::new();
    for option in options {
        let Ok(ManaPlan::Fixed(units)) = mana_plan(option) else {
            return Ok(None);
        };
        let mut pool = [0u32; 6];
        for (color, amount) in units {
            let Expr::Constant(amount) = amount else {
                return Ok(None);
            };
            let amount =
                u32::try_from(amount).map_err(|_| gap("fixed mana bundle amount exceeds u32"))?;
            let slot = &mut pool[color_index(&color)?];
            *slot = slot
                .checked_add(amount)
                .ok_or_else(|| gap("fixed mana bundle amount exceeds u32"))?;
        }
        pools.push(pool);
    }
    if pools.len() < 2 || pools.iter().all(|p| p.iter().all(|&n| n == 0)) {
        return Err(gap("empty mana bundle choice"));
    }
    Ok(Some(pools))
}

impl Compiler {
    pub(super) fn fixed_mana_bundles(
        &mut self,
        mana: &ManaProduction,
        restriction: Option<&ManaUse>,
        rest: &[Instruction],
    ) -> Result<Option<String>> {
        if !planned::switched_on("mana-bundle-choices") || !rest.is_empty() {
            return Ok(None);
        }
        // Keep the existing compact recipe for equal amounts of one color.
        if mana_plan(mana).is_ok() {
            return Ok(None);
        }
        let (lasting, restriction) = lane_g::mana_lasting(restriction);
        if lasting.is_some() {
            return Ok(None);
        }
        let Some(pools) = constant_bundles(mana)? else {
            return Ok(None);
        };
        let only_for = restriction.map(|r| self.mana_use(r)).transpose()?;
        let base = self.bundle_labels(&pools)?;
        let bundles: Vec<String> = pools.iter().map(|p| format!("ManaPool({p:?})")).collect();
        let mut adds = format!("add_one_bundle_of(&[{}], {base})", bundles.join(", "));
        if let Some(rule) = only_for {
            adds.push_str(&format!(".only_for({rule})"));
        }
        Ok(Some(adds))
    }

    fn bundle_labels(&mut self, pools: &[[u32; 6]]) -> Result<usize> {
        if self.wanted_labels.is_some()
            || (self.entering_labels.is_some() && self.choice_labels.is_empty())
        {
            return Err(gap("mana bundle choices beside other labels"));
        }
        let base = self.choice_labels.len();
        if pools.len() > 255 || base + pools.len() > 256 {
            return Err(gap("mana bundle choices exceed label indices"));
        }
        for pool in pools {
            let mut label = String::from("Add ");
            if pool.iter().all(|&n| n == 0) {
                label.push_str("no mana");
            } else {
                for (i, n) in pool.iter().copied().enumerate() {
                    if n == 0 {
                        continue;
                    }
                    let symbol = ["W", "U", "B", "R", "G", "C"][i];
                    // Avoid output proportional to a full-width constant.
                    if n <= 8 {
                        for _ in 0..n {
                            write!(label, "{{{symbol}}}").unwrap();
                        }
                    } else {
                        write!(label, "{n} {{{symbol}}}").unwrap();
                    }
                }
            }
            self.choice_labels.push(label);
        }
        let labels: Vec<String> = self.choice_labels.iter().map(|s| quoted(s)).collect();
        self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
        self.option_labels = true;
        Ok(base)
    }

    /// A spell/trigger's instruction chooses its complete bundle at resolution.
    /// Activated mana abilities instead require the authoritative native recipe:
    /// an opaque callback must not acquire incomplete payment/yield metadata.
    pub(super) fn emit_bundle_choice(
        &mut self,
        players: &PlayerSet,
        mana: &ManaProduction,
        restriction: Option<&ManaUse>,
        out: &mut String,
    ) -> Result<bool> {
        if !self.stack_item || self.pending == Pending::Entering {
            return Ok(false);
        }
        let Some(pools) = constant_bundles(mana)? else {
            return Ok(false);
        };
        let (lasting, restriction) = lane_g::mana_lasting(restriction);
        if lasting.is_some() {
            return Ok(false);
        }
        let only_for = restriction
            .map(|r| self.mana_use(r))
            .transpose()?
            .map(|rule| lane_h::rule_through_words(&rule));
        let players = self.fetch(self.players(players)?);
        let base = self.bundle_labels(&pools)?;
        let end = base + pools.len();
        let bundles: Vec<String> = pools.iter().map(|p| format!("ManaPool({p:?})")).collect();
        let id = self.next;
        self.next += 1;
        writeln!(out, "for player in {players} {{ let choice_{id} = cantrip_mechanics::choose::one_of(g, player, asker.source, ({base}usize..{end}usize).map(|i| Label::Card(item.ability.card, i as u8)).collect())?; let pool_{id} = [{}][usize::from(choice_{id})]; for mana in Mana::ALL {{ let amount = pool_{id}.get(mana); if amount > 0 {{", bundles.join(", ")).unwrap();
        match only_for {
            Some(rule) => {
                writeln!(out, "g.add_restricted_mana(player, mana, amount, {rule});").unwrap()
            }
            None => writeln!(out, "g.add_mana(player, mana, amount);").unwrap(),
        }
        out.push_str("} } }\n");
        Ok(true)
    }
}
