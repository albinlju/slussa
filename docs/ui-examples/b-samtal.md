# B · Samtal med vänstermarkering

Ingen tidslinje och inga slutna kort. Samtal avgränsas med luft, en vänsterkant
och konsekventa indrag. Fokus syns på vänsterkanten; svarens urval behåller en
egen markör. Detta går längre än A utan att införa hopfällda trådar i Overview.

## Overview

```text
  alice · author                                     12 min

  Redo för review. Felhanteringen är nu samlad.


  ┃ bob                                        Open · 8 min
  ┃ src/api.rs:42
  ┃
  ┃ 42 + return Err(error.into());
  ┃
  ┃ Kan vi behålla kontexten från det ursprungliga felet?
  ┃
  ┃   alice · author                                  5 min
  ┃   Ja, jag lägger till kontexten här.


  carol · approved                                    2 min

  j/k: comments   r: reply   R: resolve thread
```

Vanliga kommentarer utan svar har ingen dekorativ kant när de inte är
markerade. Trådar har en dämpad kant som blir tydligare vid fokus.
Metadata ligger alltid ovanför den text den hör till.

## Diff · stängd fold

```text
  40   let response = client.send().await;
  41   if let Err(error) = response {
  42 +     return Err(error.into());

       ▸ Resolved · 2 comments · bob

  43   }

  Space: expand thread   r: reply   R: reopen thread
```

## Diff · öppen fold

```text
  40   let response = client.send().await;
  41   if let Err(error) = response {
  42 +     return Err(error.into());

       ▾ Resolved · 2 comments
       ┃
       ┃ bob                                         8 min
       ┃ Kan vi behålla kontexten från det ursprungliga
       ┃ felet?
       ┃
       ┃   alice · author                            5 min
       ┃   Ja, jag lägger till kontexten här.

  43   }

  Space: collapse thread   r: reply   R: reopen thread
```

Vänsterkanten håller ihop samtalet och skiljer det från koden. Foldraden
ligger kvar på samma plats vid öppning så att man ser hur man stänger den.

## Smal Overview

```text
  ┃ bob                 Open · 8 min
  ┃ src/api.rs:42
  ┃
  ┃ Kan vi behålla kontexten från
  ┃ det ursprungliga felet?
  ┃
  ┃   alice · author           5 min
  ┃   Ja, jag lägger till kontexten
  ┃   här.

  j/k: comments   r: reply
```

Även här utelämnas kodförhandsvisningen när utrymmet är litet. Svarens indrag
hålls kort för att inte orsaka onödigt många radbrytningar.

**Styrka:** texten får mest uppmärksamhet, få visuella element.

**Avvägning:** långa samtal kan vara svårare att skilja åt. Luft kostar också
höjd, även om antalet linjer minskar.
