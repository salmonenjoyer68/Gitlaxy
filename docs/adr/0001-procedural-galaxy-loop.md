# Procedural galaxy loop

The supplied galaxy sheets cannot supply clean animation frames: the cells do
not sit on a grid, the fifth stage is clipped at the canvas edge in five of six
colorways, and neither sheet has an alpha channel. Animating the raster art would
therefore require an outpainting pass and would still seam badly when rotated.

The looping GIF is drawn procedurally instead: spiral arms, stars, dust, and core
glow are generated from the account seed, the colorway palette, and the derived
stage. This gives clean transparency, all five stages, and a seamless rotation
and twinkle loop. The galaxy sheets remain the reference art for palettes and
the README illustration, not the loop's frames.

The dust mote stream advances once per Loop and wraps exactly: each mote falls
from the rim to the core and reappears at the rim on the next Loop. Its
brightness is zero at both ends, so the respawn and the seam show no jump.
Quiet seeds fewer motes and dims them with the rest of the galaxy.
