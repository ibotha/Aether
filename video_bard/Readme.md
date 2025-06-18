# Video Bard

The video bard will be in charge of GPU friendly operations.

I am not sure where to draw the line yet but for now this will include:
- Computer vision implemented as compute shaders
- Projection onto the real world

The plan here is to start with debug tools to easily display images, videos, and simple shapes (maybe text?) at desired locations. Bootstrapping our way to a system that can analyze video streams in real time and display useful graphics.

After that we can iterate on that system to be calibrated properly in the real worlds by responding to stimulus from the camera. That should be all we need on that side.

This, like all bards should be designed with the Aether in mind. [Notes on bard design](../Readme.md#bard-design)