## Overview

Basic idea would be:
- Editor features are implemented in the shine-game crate and uses bevy
- Godot is a "render" layer and event based updates are driven by the bevy.
  - Camera is a godot entity moved by godot tools
  - Godot provides events for bevy about the current target location
  - Bevy triggers (bevy) chunk entity creation
  - The client adds a GD component to the bevy entity to connect bevy and GD world
  - GD objects are spawned as a child for the world with transformation, bevy has no (world) transformation