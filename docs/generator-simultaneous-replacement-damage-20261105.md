# Additional damage in a replacement

Damage that a replacement adds belongs to the modified simultaneous event
(CR 120.4b–d, 614.6). Emitting an ordinary `.then` callback that calls
`Game::damage_together` creates a separate event instead. Phyrexian Unlife
distinguishes these behaviors when both components damage the same player.

The backend now emits pure `DamageComponent::Additional` callbacks through
`.with_damage` for typed damage-only replacement programs. Both printed and
granted replacements use this path. Groups preserve simultaneity; programs that
mix additional damage with other instructions retain an explicit gap until they
can be represented faithfully. The separate
`simultaneous-replacement-damage` capability requires the native component API;
wide numeric arguments alone do not establish support. Ordinary damage outside
a replacement keeps its existing emission path.

The saved Eye for an Eye regression failed with the previous generated module:
a player starting at 1 life with Phyrexian Unlife finished at −1 life and 2 poison.
The freshly generated module finishes at −3 life and 0 poison, and all 10 retained
Eye scenarios pass on native foundation `7c77eca06`. This native probe establishes
the generator correction; it does not replace the card lane's independent review
and confirmation. Five focused generator tests cover final PR form, both callback
signatures, the absent capability, grouping and mixed instructions. The full Rust
workspace passes 1,558 tests, with four existing ignored tests.
