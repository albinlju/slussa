# A · Mjukare original

Behåll kommentarernas tydliga avgränsning och den befintliga idén med en
markerad vänsterkant. Ta bort den extra tidslinjen utanför korten, upprepade
etiketter och ramar inuti ramar. Namn och text får vara viktigare än status.

## Overview

```text
  alice · author                                     12 min
  ╭────────────────────────────────────────────────────────╮
  │ Redo för review. Felhanteringen är nu samlad.            │
  ╰────────────────────────────────────────────────────────╯

  bob                                          Open · 8 min
  src/api.rs:42
  ╭────────────────────────────────────────────────────────╮
  ┃ 42 + return Err(error.into());                          │
  ┃                                                        │
  ┃ Kan vi behålla kontexten från det ursprungliga felet?    │
  ┃                                                        │
  ┃ alice · author                                   5 min │
  ┃ Ja, jag lägger till kontexten här.                      │
  ╰────────────────────────────────────────────────────────╯

  carol · approved                                    2 min

  j/k: comments   r: reply   R: resolve thread
```

`┃` representerar den fokuserade trådens ljusare vänsterkant. Övriga kanter
är dämpade. Det behövs ingen färgad bakgrund över hela kortet.
Val av enskilda svar behåller sin separata markör vid namnet.

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
       ╭──────────────────────────────────────────────────╮
       ┃ bob                                        8 min │
       ┃ Kan vi behålla kontexten från det ursprungliga    │
       ┃ felet?                                           │
       ┃                                                  │
       ┃ alice · author                             5 min │
       ┃ Ja, jag lägger till kontexten här.                │
       ╰──────────────────────────────────────────────────╯

  43   }

  Space: collapse thread   r: reply   R: reopen thread
```

## Smal Overview

```text
  bob                   Open · 8 min
  src/api.rs:42
  ╭────────────────────────────────╮
  ┃ Kan vi behålla kontexten från   │
  ┃ det ursprungliga felet?         │
  ┃                                │
  ┃ alice · author           5 min │
  ┃ Ja, jag lägger till kontexten   │
  ┃ här.                           │
  ╰────────────────────────────────╯

  j/k: comments   r: reply
```

I smala fönster utelämnas kodförhandsvisningen i Overview; filplatsen finns
kvar och hela diffen finns i Diff-fliken. Kommentarstexten radbryts.

**Styrka:** tydliga grupper, nära det du redan gillade.

**Avvägning:** ramar tar fortfarande både utrymme och uppmärksamhet.
