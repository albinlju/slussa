# Graphite

Standardtemat i tuipr, med en neutral palett för mörka terminaler.

```bash
cargo run --release
```

Text, författare, filplatser, rubriker, labels och commit-ID:n använder gråtoner.
Blått markerar interaktion: fokus, vald flik, dialoger och aktiva handlingar.
Status och diff behåller grönt, korall, gult och violett, men med lägre mättnad.
Markerade rader har neutral grå bakgrund i stället för Slates blågrå ton.
Terminalens vanliga bakgrund behålls. Temat är avsett för mörka bakgrunder.

| Roll | Färg |
| --- | --- |
| Text | `#E6E6E3` |
| Namn, rubriker, labels | `#D3D4D5` |
| Filplatser och länkar | `#C3C5C8` |
| Metadata / kontext | `#A3A4A6` |
| Fokus | `#81B4F4` |
| Tillagt / godkänt | `#91B79B` |
| Borttaget / fel | `#D99898` |
| Varning | `#CEB581` |
| Suggestion / merged | `#B0A3C5` |
| Avdelare | `#4A4C50` |
| Ram | `#686B70` |
| Markerad rad | `#343638` |

Övriga teman finns kvar via `theme = "…"` i konfigurationen eller `TUIPR_THEME`.
Exempelvis väljer `TUIPR_THEME=terminal` terminalens egen palett.
Skillnaden ligger framför allt i att passivt innehåll inte använder fokusfärgen.
