# C · Kompakt trådlista

Overview blir en skannbar lista där man öppnar samtal vid behov. Detta är ett
större experiment: det ändrar hur man arbetar, inte bara hur kommentarerna ser ut.
Markering och öppet/stängt läge är separata så att navigation inte får innehåll
att hoppa upp och ned automatiskt.

## Overview · överblick

```text
    alice · author                                   12 min
    Redo för review. Felhanteringen är nu samlad.

  › ▸ bob · src/api.rs:42                      Open · 8 min
      Kan vi behålla kontexten från det ursprungliga felet?
      2 comments

    carol · approved                                  2 min

  j/k: comments   Enter: expand thread   r: reply
```

`›` betyder markerad rad. `▸` betyder stängd tråd. Förhandsvisningen är
kommentarens första textrad och kapas med `…` vid behov. Antalet inkluderar
både första kommentaren och svaren.

## Overview · öppet samtal

```text
    alice · author                                   12 min
    Redo för review. Felhanteringen är nu samlad.

  › ▾ bob · src/api.rs:42                      Open · 8 min
      42 + return Err(error.into());

      Kan vi behålla kontexten från det ursprungliga felet?

      alice · author                                  5 min
      Ja, jag lägger till kontexten här.

    carol · approved                                  2 min

  Enter: collapse thread   r: reply   R: resolve thread
```

Trådar öppnas uttryckligen med Enter och behåller sitt läge när man navigerar.
Urval av enskilda svar används inne i den öppna tråden. Hopfällda svar måste
öppnas innan man kan markera dem för exempelvis redigering eller borttagning.

## Diff · stängd fold

```text
  40   let response = client.send().await;
  41   if let Err(error) = response {
  42 +     return Err(error.into());

     › ▸ Resolved · 2 comments
         Kan vi behålla kontexten från det ursprungliga…

  43   }

  Space: expand thread   r: reply   R: reopen thread
```

En extra förhandsrad ger ledtråd om innehållet innan man öppnar. Den kostar
en rad per fold jämfört med A och B. Öppna, olösta difftrådar fortsätter att
visa hela samtalet som standard så att väntande diskussioner inte döljs.

## Diff · öppen fold

```text
  40   let response = client.send().await;
  41   if let Err(error) = response {
  42 +     return Err(error.into());

     › ▾ Resolved · 2 comments
         bob                                          8 min
         Kan vi behålla kontexten från det ursprungliga
         felet?

         alice · author                               5 min
         Ja, jag lägger till kontexten här.

  43   }

  Space: collapse thread   r: reply   R: reopen thread
```

## Smal Overview

```text
  › ▸ bob               Open · 8 min
      src/api.rs:42
      Kan vi behålla kontexten…
      2 comments

    carol · approved          2 min

  Enter: expand thread   r: reply
```

**Styrka:** många diskussioner kan överblickas utan en lång vägg av text.

**Avvägning:** fler öppningar krävs för att läsa allt. De två markörerna och
skillnaden mellan markering och expansion behöver vara begripliga. Passar
sämre om målet främst är att läsa alla kommentarer från början till slut.
