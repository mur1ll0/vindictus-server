import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { PreviewModel } from '@/lib/vindictusModel';

type Props = { model: PreviewModel; showBones: boolean; wireframe: boolean };

/** Orbit preview of a model kit: textured LOD 0 meshes, optional bones and wireframe. */
export function ModelViewer({ model, showBones, wireframe }: Props) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setSize(element.clientWidth, element.clientHeight);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    element.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    scene.add(new THREE.HemisphereLight(0xffffff, 0x404050, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.4);
    sun.position.set(1, 2, 3);
    scene.add(sun);

    // The game is Z-up; three.js is Y-up.
    const root = new THREE.Group();
    root.rotation.x = -Math.PI / 2;
    scene.add(root);

    const textures = new Map<string, THREE.DataTexture>();
    for (const [name, t] of model.textures) {
      const texture = new THREE.DataTexture(t.rgba, t.width, t.height, THREE.RGBAFormat);
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
      texture.magFilter = THREE.LinearFilter;
      texture.minFilter = THREE.LinearMipmapLinearFilter;
      texture.generateMipmaps = true;
      texture.needsUpdate = true;
      textures.set(name, texture);
    }
    const disposables: { dispose(): void }[] = [...textures.values()];
    for (const mesh of model.meshes) {
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(mesh.positions, 3));
      geometry.setAttribute('normal', new THREE.BufferAttribute(mesh.normals, 3));
      // VTF rows are stored top first and the game's V grows downwards: both flips cancel out.
      geometry.setAttribute('uv', new THREE.BufferAttribute(mesh.uvs, 2));
      geometry.setIndex(new THREE.BufferAttribute(mesh.indices, 1));
      const material = new THREE.MeshStandardMaterial({
        map: textures.get(mesh.material) ?? null,
        color: textures.has(mesh.material) ? 0xffffff : 0xb0a090,
        roughness: 0.8,
        side: THREE.DoubleSide,
        wireframe,
      });
      disposables.push(geometry, material);
      root.add(new THREE.Mesh(geometry, material));
    }
    if (showBones) {
      const points: number[] = [];
      for (const bone of model.bones) {
        const parent = model.bones[bone.parent];
        if (!parent) continue;
        points.push(...parent.position, ...bone.position);
      }
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.Float32BufferAttribute(points, 3));
      const material = new THREE.LineBasicMaterial({ color: 0xff4fa3, depthTest: false });
      const lines = new THREE.LineSegments(geometry, material);
      lines.renderOrder = 1;
      disposables.push(geometry, material);
      root.add(lines);
    }

    const box = new THREE.Box3().setFromObject(root);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3()).length() || 1;
    const camera = new THREE.PerspectiveCamera(35, element.clientWidth / element.clientHeight, size / 100, size * 20);
    camera.position.copy(center).add(new THREE.Vector3(0, size * 0.1, size * 1.4));
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.copy(center);
    controls.update();

    let frame = 0;
    const render = () => {
      frame = requestAnimationFrame(render);
      controls.update();
      renderer.render(scene, camera);
    };
    render();
    const resize = new ResizeObserver(() => {
      camera.aspect = element.clientWidth / Math.max(1, element.clientHeight);
      camera.updateProjectionMatrix();
      renderer.setSize(element.clientWidth, element.clientHeight);
    });
    resize.observe(element);
    return () => {
      cancelAnimationFrame(frame);
      resize.disconnect();
      controls.dispose();
      disposables.forEach((d) => d.dispose());
      renderer.dispose();
      element.removeChild(renderer.domElement);
    };
  }, [model, showBones, wireframe]);

  return <div ref={host} className="h-[560px] w-full overflow-hidden rounded-lg border border-border bg-muted/30" />;
}
