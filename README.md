# Blur

Cross-platform preservation, compatibility and customization project for a
locally owned copy of Blur (Bizarre Creations, 2010).

Original game data stays immutable. `work/game-overlay` combines symlinks to
the mounted data with the compatible executable selected by checksum. Native
ports, Wine prefixes, saves and generated code remain local under `work/` or in
their upstream ignored build directories.

Two complementary runtimes are under evaluation:

1. A clean Wine 32-bit prefix for the authentic native Windows path.
2. `blur-web`, which lifts the supported x86 executable to native code and
   exposes rendering/input through a browser without duplicating game logic.
