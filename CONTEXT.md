# Gitlaxy

A profile README art project that turns a GitHub account's public activity into
a pixel-art galaxy.

## Language

**Galaxy**:
The single subject of a render: one spiral of stars on a square canvas.
_Avoid_: koi, fish, object

**Galaxy sheet**:
The supplied dark and light contact sheets showing six colorways at five
stages; reference art for palettes and the README illustration, not render
frames.
_Avoid_: sprite sheet, tileset

**Colorway**:
One of six fixed palettes — blue, red, orange, purple, green, pink — chosen by
the user and independent of activity.
_Avoid_: body, skin, theme

**Stage**:
One of five growth sizes of a galaxy, derived from the account's total
contributions.
_Avoid_: level, tier, zoom

**Motion state**:
One of two animation modes, `Quiet` or `Active`, selected from the account's
recent activity.
_Avoid_: behavior, mood, animation type

**Flare**:
A short bright pulse in an `Active` loop, fixed at a seeded loop phase.
_Avoid_: burst, flash

**Comet trail**:
A glint trail along one arm whose length reflects the account's current
contribution streak.
_Avoid_: streak line, tail

**Pigment**:
A language-derived accent color, bucketed into red, orange, yellow, or blue
(grays become white or gold), applied to some stars and arm accents.
_Avoid_: language color, tint

**Snapshot**:
A record of an account's public repositories, language colors, and daily
contributions.
_Avoid_: dump, export, profile JSON

**Theme variant**:
The dark-page or light-page version of the same render.
_Avoid_: dark mode, background, body

**Still**:
The single PNG of one account's galaxy at its stage and colorway.
_Avoid_: static image, frame

**Loop**:
The seamless animated GIF of one account's galaxy at its stage and colorway;
it shows motion, not growth.
_Avoid_: idle loop, video, clip, swim
