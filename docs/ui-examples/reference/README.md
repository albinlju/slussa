# Bevarade referenser

Originalet före experimenten finns i commit `6edd115` (UI improvements).
Filerna som styr presentationen är:

- `src/tui/widgets/comment.rs`
- `src/tui/screens/pr_detail/tabs/overview/timeline.rs`
- `src/tui/screens/pr_detail/tabs/overview/mod.rs`

`first-experiment.patch` sparar den första, avskalade varianten som en diff
mot detta original. Variant A byggs vidare i appens vanliga källfiler.
Originalet är referensen om vi vill behålla dess layout och bara justera färger.
Återställning ska göras selektivt för dessa presentationsdelar, så att senare
funktionella ändringar och tester inte skrivs över.

## Variant A är sparad

`variant-a.patch` sparar A relativt commit `6edd115`, inklusive tester,
dokumentation och den befolkade Overview-snapshoten. `variant-a-preview.txt`
visar layouten vid 100, 80 och 40 kolumner. Patchen är en återställningsreferens;
applicera den på basversionen eller jämför selektivt mot senare ändringar.

Den aktiva varianten är nu originalets layout med färgjusteringar: accent på
fokuserad vänsterkant och svarmarkör, separat länkfärg för filplatsen och
konsekvent grön resolved-status även när folden är markerad. Originalets
avstånd, ramar, tidslinje, etiketter och sidebar-brytpunkt är återställda.
