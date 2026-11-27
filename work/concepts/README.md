# From concept art to Shipvivor assets

The forecastle captain and the flagship repair berth are good first trials for different reasons. The captain needs to look like the same person as she turns her head. The berth needs a crane, ship and halls that look right together and can move or disappear independently. The workflows below are candidates as of November 2026.

## The forecastle captain

**Meshy** can turn a full-body reference into a textured humanoid with a skeleton, skin weights and sample motion. Start with front, side and back views of the same uniform in a relaxed A-pose. Leave space between each arm and the coat so the generator can see the sleeve boundary. If the scarf or coat comes out fused to the body, try **Tripo** on the same views. Its part separation and mesh-density controls make it worth comparing on time spent fixing the result in Blender. [Full-body painting](../../src/captain/prow.png) · [Meshy rigging example](https://www.meshy.ai/developers/cookbook/recipes/rig-3d-character/) · [Tripo tools](https://www.tripo3d.ai/help/getting-started/what-features-does-tripo-have)

**HairDeck** builds individual ribbons of hair from a portrait and generates the bone chains that move them. Use the bareheaded captain in the left panel so her cap from later portraits cannot become part of the hair. The hair-only GLB is useful for inspecting shape and colour. The rigged VRM on the template avatar is the starting point for fitting the moving locks to her head in Blender. Check the ivory forelock in profile and the clearance between her cheek and the side locks through a head turn. [Captain portraits](breakwater-captain.png) · [HairDeck](https://hyzcluster.github.io/HairDeck/) · [Export formats](https://github.com/hyz317/HairDeck#-outputs)

**In-game trial.** In Blender, fit the figure to the existing 1.90 m deck placement and bake one loop in which she turns and braces. Export a GLB with the textures and clip. Shipvivor would need Bevy’s glTF loader, animation support and the codecs for the embedded textures. Compare the loop with the painted figure at title-screen size. Watch the boots for sliding and the scarf for clipping through the shoulder. The helm portrait already has its own painted expressions. [Forecastle figure](../../src/captain/prow.rs) · [Current title shot](../visualizations/captain-prow.jpg) · [Bevy build features](../../Cargo.toml)

## The repair berth

**CoLa3D** can turn a scene image into separate object meshes that share a coordinate frame. Crop one panel of the berth painting and mark the crane, dock, ship and each furnace hall with separate masks. The full pipeline can export a textured GLB per object. In Blender, hide the ship and inspect the exposed piers from water level, then move the crane along the dock. Thin, adjacent pieces can merge, so keep the existing animated hoist wires and plate and fit the generated crane around them. The authors document a Linux and NVIDIA CUDA setup. [Berth painting](flagship-repair-berth.png) · [Harbour geometry](../../src/harbour_models.rs) · [CoLa3D examples](https://physicalvision.github.io/CoLa3D/) · [Inference code](https://github.com/Master-cai/CoLa3D)

## Previewing a silhouette

**Budget3D** turns one image into a Gaussian splat that can be viewed from different angles. An orbit of the captain can help settle the shape of her coat and scarf before drawing the turnaround. Treat the unseen side as a design suggestion. The budget slider changes generation time and memory use. The exported `.ply` contains splats, so a deforming character still needs a surface mesh and rig. [Budget3D gallery and demo](https://budgetany.github.io/budget3d/) · [Sampler and output settings](https://github.com/BudgetAny/Budget3D/blob/main/scripts/sample.py)
