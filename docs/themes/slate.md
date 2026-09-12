# Slate

Ett förslag till tuiprs eget färgtema för mörka terminalbakgrunder.
Standardbakgrunden i terminalen behålls. Appens layout och kortkommandon ändras inte.

```bash
TUIPR_THEME=slate cargo run --release
```

För att välja det permanent kan den befintliga konfigurationen innehålla
`theme = "slate"`. På macOS finns filen normalt i `~/.config/tuipr/config.toml`;
`XDG_CONFIG_HOME` har företräde. Miljövariabeln har företräde över filen.

[Öppna färgförhandsvisningen](slate-preview.html). Den visar illustrativa exempel,
inte en skärmbild från appen. Bakgrundsvalen i förhandsvisningen ändrar bara sidan.

| Roll | Färg | Avsikt |
| --- | --- | --- |
| Text | `#DEE3EA` | Neutral, utan blå eller gul ton som dominerar |
| Metadata | `#9AA6B5` | Sekundär men fortfarande lättläst |
| Namn och information | `#B7C8DA` | Skilja författare från text utan att se ut som fokus |
| Fokus | `#87BFFF` | Tydlig kallblå markering |
| Länkar och filplatser | `#91BDE0` | Lugnare blå än den aktiva markeringen |
| Godkänt / tillagt | `#8BC6A0` | Mjuk grön |
| Fel / borttaget | `#EE9797` | Korall, skild från den gröna även i ljushet |
| Varning / väntande | `#DEBC7C` | Varmt guld |
| Suggestion / merged | `#B3A2D8` | Återhållsam violett |
| Avdelare | `#465261` | Struktur som ligger bakom innehållet |
| Ram | `#65758A` | Tydligare avgränsning än interna linjer |
| Markerad rad | `#293849` | En diskret blågrå yta |

Utgångspunkten är en tydlig hierarki, inte maximal färgmättnad. Status har fortsatt
text och symboler; färg är inte den enda ledtråden. På de kontrollerade mörka
bakgrunderna `#181C22`, `#282828` och `#1E1E2E` är textens beräknade kontrast minst
11,4:1 och metadatans minst 5,9:1. På markerad rad är motsvarande värden 9,3:1 och
4,8:1. Detta är beräkningar för dessa färgpar, inte ett löfte om alla terminaler
eller en fullständig tillgänglighetsgranskning.

Det är ett mörkt tema. Terminal-läget finns kvar för exempelvis ljusa terminaler.
Slate har lagts till som ett alternativ; det är ännu inte standardtemat.
