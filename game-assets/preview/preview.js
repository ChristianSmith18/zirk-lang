import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';
import { CATALOG, CATEGORIES, byCategory, byId, createLights, createSky, createStars, createTile, createShadowBlob, PALETTE } from 'zirk-assets';

const params = new URLSearchParams(location.search);
const shot = params.has('shot');
if (shot) document.body.classList.add('shot');

const app = document.getElementById('app');
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: shot });
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.shadowMap.enabled = true;
renderer.shadowMap.type = THREE.PCFSoftShadowMap;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.05;
app.appendChild(renderer.domElement);

const scene = new THREE.Scene();
scene.add(createSky(), createStars());
scene.fog = new THREE.Fog(PALETTE.deepViolet, 24, 60);
const { group: lights } = createLights();
scene.add(lights);

const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 200);
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.maxPolarAngle = Math.PI * 0.49;

let stage = new THREE.Group();
scene.add(stage);
const animated = [];
let current = params.get('category') ?? 'characters';

function label(text) {
  const canvas = document.createElement('canvas');
  canvas.width = 512;
  canvas.height = 96;
  const ctx = canvas.getContext('2d');
  ctx.font = '600 44px "Space Grotesk", system-ui, sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillStyle = '#f7f7fa';
  ctx.fillText(text, 256, 48);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: texture, transparent: true, depthTest: false }));
  sprite.scale.set(1.6, 0.3, 1);
  sprite.renderOrder = 10;
  return sprite;
}

function build(category) {
  scene.remove(stage);
  stage = new THREE.Group();
  scene.add(stage);
  animated.length = 0;

  // `?asset=<id>` shows one asset large, for close inspection.
  const single = params.get('asset');
  const items = single ? [byId(single)] : byCategory(category);
  const columns = single ? 1 : Math.ceil(Math.sqrt(items.length * 1.7));
  const rows = Math.ceil(items.length / columns);
  const spacing = category === 'teaching' ? 3.1 : 1.8;
  items.forEach((item, index) => {
    const col = index % columns;
    const row = Math.floor(index / columns);
    const cell = new THREE.Group();
    cell.position.set((col - (columns - 1) / 2) * spacing, 0, (row - (rows - 1) / 2) * spacing);
    if (category !== 'world') cell.add(createTile('circuit', { variant: index }));
    const asset = item.create();
    cell.add(asset);
    if (!['effects', 'world', 'teaching'].includes(category)) cell.add(createShadowBlob({ radius: 0.32 }));
    const text = label(item.label);
    text.position.set(0, category === 'world' ? 0.7 : 1.4, 0);
    cell.add(text);
    stage.add(cell);
    if (asset.update) animated.push(asset);
    // A short, varied animation per bot so the sheet shows the whole set.
    if (asset.play) asset.play(['idle', 'walk', 'celebrate', 'think', 'sad', 'jump'][index % 6], { loop: true });
    // Demonstrate the state-driven assets in a mid-way pose.
    asset.setField?.(0, 0.8);
    asset.setField?.(1, 0.45);
    asset.setField?.(2, 0.95);
    asset.setLevel?.(0.7);
    asset.open?.(0.55);
    asset.unroll?.(1);
    asset.setProgress?.(0.5);
  });

  if (single) {
    camera.position.set(0.55, 1.05, 1.45);
    controls.target.set(0, 0.5, 0);
    controls.update();
    return;
  }

  // Fit the camera to the laid-out stage.
  const bounds = new THREE.Box3().setFromObject(stage);
  const size = bounds.getSize(new THREE.Vector3());
  const radius = Math.max(size.x, size.z) * 0.62 + 1.2;
  const distance = radius / Math.tan(THREE.MathUtils.degToRad(camera.fov / 2)) * (camera.aspect < 1 ? 1.5 : 0.5);
  camera.position.set(distance * 0.42, distance * 0.62, distance * 0.78);
  controls.target.set(0, 0.45, 0);
  controls.update();
}

function resize() {
  const { clientWidth: w, clientHeight: h } = app;
  renderer.setSize(w, h);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}
addEventListener('resize', resize);

const nav = document.querySelector('nav');
for (const category of CATEGORIES) {
  const button = document.createElement('button');
  button.textContent = `${category} (${byCategory(category).length})`;
  button.setAttribute('aria-pressed', String(category === current));
  button.addEventListener('click', () => {
    current = category;
    nav.querySelectorAll('button[data-cat]').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.cat === category)));
    build(category);
  });
  button.dataset.cat = category;
  nav.appendChild(button);
}
const exportButton = document.createElement('button');
exportButton.textContent = 'Export GLB';
exportButton.addEventListener('click', async () => {
  const exporter = new GLTFExporter();
  // Fresh instances in a row: `Object3D.clone()` would rebuild the articulated
  // assets (they construct their own children) and duplicate their parts.
  const root = new THREE.Group();
  byCategory(current).forEach((item, index) => {
    const asset = item.create();
    asset.name = item.id;
    asset.position.x = index * 1.6;
    root.add(asset);
  });
  const glb = await exporter.parseAsync(root, { binary: true });
  const url = URL.createObjectURL(new Blob([glb], { type: 'model/gltf-binary' }));
  const link = Object.assign(document.createElement('a'), { href: url, download: `zirk-${current}.glb` });
  link.click();
  URL.revokeObjectURL(url);
});
nav.appendChild(exportButton);
nav.insertAdjacentHTML('beforeend', `<span class="hint">${CATALOG.length} assets · drag to orbit</span>`);

resize();
build(current);

const clock = new THREE.Clock();
const warm = Number(params.get('warm') ?? 0);
for (let i = 0; i < warm; i++) animated.forEach((a) => a.update(1 / 60));

function frame() {
  const dt = Math.min(clock.getDelta(), 0.05);
  animated.forEach((asset) => asset.update(dt));
  controls.update();
  renderer.render(scene, camera);
  requestAnimationFrame(frame);
}
frame();
window.__ready = true;
