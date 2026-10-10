import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { solid, glow, ghost } from '../materials.js';
import { Asset, box, cylinder, cone, sphere, roundBox, mesh } from '../shapes.js';

/**
 * Looks of the bot family. Each kind is a different "class" in the lessons:
 * same skeleton (`Bot`), different state and behavior. Colors come from the
 * palette so a kind reads at a glance, even in silhouette.
 */
export const BOT_KINDS = Object.freeze({
  zirk: { label: 'Zirk (mascot)', body: PALETTE.violet, trim: PALETTE.deepViolet, visor: PALETTE.surface, eyes: PALETTE.cyan, bulb: PALETTE.pink },
  scout: { label: 'Scout', body: PALETTE.cyan, trim: PALETTE.deepViolet, visor: PALETTE.surface, eyes: PALETTE.text, bulb: PALETTE.aqua },
  builder: { label: 'Builder', body: PALETTE.aqua, trim: PALETTE.stoneDark, visor: PALETTE.surface, eyes: PALETTE.warning, bulb: PALETTE.warning },
  guard: { label: 'Guard', body: PALETTE.pink, trim: PALETTE.deepViolet, visor: PALETTE.surface, eyes: PALETTE.text, bulb: PALETTE.error },
  courier: { label: 'Courier', body: PALETTE.warning, trim: PALETTE.woodDark, visor: PALETTE.surface, eyes: PALETTE.deepViolet, bulb: PALETTE.cyan },
});

const ANIMATIONS = ['idle', 'walk', 'jump', 'celebrate', 'sad', 'think'];

/** A small articulated robot. Total height is about 1.0 (one tile). */
export class Bot extends Asset {
  constructor(kind = 'zirk', { ghosted = false, tint } = {}) {
    super(`bot:${kind}`);
    const look = BOT_KINDS[kind];
    if (!look) throw new Error(`unknown bot kind "${kind}"; expected one of ${Object.keys(BOT_KINDS).join(', ')}`);
    this.botKind = kind;
    this.userData.label = look.label;
    this._state = { name: 'idle', t: 0, loop: true };
    this._facing = 0;

    const bodyMat = ghosted ? ghost(tint ?? look.body, 0.4) : solid(look.body);
    const trimMat = ghosted ? ghost(tint ?? look.body, 0.25) : solid(look.trim, { roughness: 0.8 });
    const visorMat = ghosted ? ghost(PALETTE.text, 0.35) : solid(look.visor, { roughness: 0.25, metalness: 0.4 });
    const eyeMat = ghosted ? ghost(PALETTE.text, 0.8) : glow(look.eyes, 1.8);
    const bulbMat = ghosted ? ghost(PALETTE.text, 0.8) : glow(look.bulb, 2);

    this.rig = new THREE.Group();
    this.add(this.rig);

    // Legs hang from hip pivots so they can swing.
    this.legL = this._limb(-0.12, 0.3, 0, cylinder(0.075, 0.075, 0.28, 8), trimMat, [0, -0.14, 0]);
    this.legR = this._limb(0.12, 0.3, 0, cylinder(0.075, 0.075, 0.28, 8), trimMat, [0, -0.14, 0]);
    for (const leg of [this.legL, this.legR]) {
      leg.add(mesh(roundBox(0.17, 0.07, 0.24, 0.03), bodyMat, [0, -0.29, 0.03]));
    }

    // Torso and head.
    this.torso = mesh(roundBox(0.46, 0.38, 0.32, 0.07), bodyMat, [0, 0.49, 0]);
    this.rig.add(this.torso);
    this.head = new THREE.Group();
    this.head.position.set(0, 0.93, 0);
    this.rig.add(this.head);
    this.head.add(mesh(roundBox(0.52, 0.4, 0.42, 0.09), bodyMat, [0, 0, 0]));
    this.head.add(mesh(roundBox(0.4, 0.24, 0.04, 0.05), visorMat, [0, 0.01, 0.2], { shadow: false }));
    this.eyes = new THREE.Group();
    this.eyes.position.set(0, 0.02, 0.225);
    for (const x of [-0.09, 0.09]) this.eyes.add(mesh(box(0.07, 0.11, 0.02), eyeMat, [x, 0, 0], { shadow: false }));
    this.head.add(this.eyes);

    // Antenna with a pulsing bulb.
    this.head.add(mesh(cylinder(0.015, 0.015, 0.16, 6), trimMat, [0, 0.28, 0]));
    this.bulb = mesh(sphere(0.045, 10), bulbMat, [0, 0.38, 0], { shadow: false });
    this.head.add(this.bulb);

    // Arms hang from shoulder pivots.
    this.armL = this._limb(-0.29, 0.62, 0, roundBox(0.1, 0.3, 0.1, 0.04), trimMat, [0, -0.13, 0]);
    this.armR = this._limb(0.29, 0.62, 0, roundBox(0.1, 0.3, 0.1, 0.04), trimMat, [0, -0.13, 0]);

    this._extras(kind, look, { bodyMat, trimMat, bulbMat, eyeMat });
    this.traverse((o) => {
      if (o.isMesh) o.castShadow = !ghosted;
    });
  }

  _limb(x, y, z, geometry, material, offset) {
    const pivot = new THREE.Group();
    pivot.position.set(x, y, z);
    pivot.add(mesh(geometry, material, offset));
    this.rig.add(pivot);
    return pivot;
  }

  _extras(kind, look, { bodyMat, trimMat, bulbMat, eyeMat }) {
    switch (kind) {
      case 'zirk': {
        // A "Z" on the chest: two bars and a diagonal.
        const z = new THREE.Group();
        z.position.set(0, 0.5, 0.165);
        z.add(mesh(box(0.2, 0.04, 0.02), eyeMat, [0, 0.09, 0], { shadow: false }));
        z.add(mesh(box(0.2, 0.04, 0.02), eyeMat, [0, -0.09, 0], { shadow: false }));
        z.add(mesh(box(0.27, 0.04, 0.02), eyeMat, [0, 0, 0], { rotation: [0, 0, 0.78], shadow: false }));
        this.rig.add(z);
        for (const x of [-0.275, 0.275]) this.head.add(mesh(cylinder(0.07, 0.07, 0.06, 10), solid(PALETTE.cyan), [x, 0, 0], { rotation: [0, 0, Math.PI / 2] }));
        break;
      }
      case 'scout': {
        this.head.add(mesh(cone(0.12, 0.1, 10), solid(PALETTE.text, { metalness: 0.3 }), [0, 0.47, 0], { rotation: [Math.PI, 0, 0] }));
        this.rig.add(mesh(box(0.34, 0.05, 0.02), bulbMat, [0, 0.52, 0.165], { shadow: false }));
        break;
      }
      case 'builder': {
        const hat = new THREE.Group();
        hat.position.set(0, 0.22, 0);
        hat.add(mesh(cylinder(0.2, 0.26, 0.14, 12), solid(PALETTE.warning), [0, 0.06, 0]));
        hat.add(mesh(box(0.56, 0.03, 0.46), solid(PALETTE.warning), [0, -0.01, 0.02]));
        this.head.add(hat);
        // A wrench in the right hand.
        this.armR.add(mesh(box(0.05, 0.26, 0.05), solid(PALETTE.stone), [0.02, -0.34, 0.06], { rotation: [0.4, 0, 0] }));
        this.armR.add(mesh(torus3(0.06), solid(PALETTE.stone), [0.02, -0.46, 0.12], { rotation: [0.4, 0, 0] }));
        break;
      }
      case 'guard': {
        for (const x of [-0.31, 0.31]) this.rig.add(mesh(roundBox(0.14, 0.12, 0.36, 0.04), solid(PALETTE.deepViolet), [x, 0.7, 0]));
        this.rig.add(mesh(box(0.26, 0.07, 0.02), solid(PALETTE.warning, { emissive: PALETTE.warning, emissiveIntensity: 0.8 }), [0, 0.5, 0.165], { shadow: false }));
        this.head.add(mesh(box(0.1, 0.12, 0.34), solid(PALETTE.deepViolet), [0, 0.25, 0]));
        break;
      }
      case 'courier': {
        this.rig.add(mesh(roundBox(0.38, 0.4, 0.2, 0.05), solid(PALETTE.woodDark), [0, 0.5, -0.26]));
        this.rig.add(mesh(box(0.4, 0.05, 0.22), solid(PALETTE.warning), [0, 0.6, -0.26]));
        break;
      }
      default:
        break;
    }
  }

  /** Face the given heading (radians around +Y; 0 looks toward +Z). */
  face(angle) {
    this._facing = angle;
    this.rig.rotation.y = angle;
    return this;
  }

  /** Start an animation. One-shots (`jump`, `celebrate`) fall back to idle. */
  play(name, { loop } = {}) {
    if (!ANIMATIONS.includes(name)) throw new Error(`unknown animation "${name}"; expected one of ${ANIMATIONS.join(', ')}`);
    const oneShot = name === 'jump' || name === 'celebrate';
    this._state = { name, t: 0, loop: loop ?? !oneShot };
    return this;
  }

  get animation() {
    return this._state.name;
  }

  animate(dt) {
    const s = this._state;
    s.t += dt;
    const t = s.t;
    // Reset the pose each frame, then layer the animation on top.
    this.rig.position.y = 0;
    this.rig.rotation.set(0, this._facing, 0);
    this.legL.rotation.set(0, 0, 0);
    this.legR.rotation.set(0, 0, 0);
    this.armL.rotation.set(0, 0, 0);
    this.armR.rotation.set(0, 0, 0);
    this.head.rotation.set(0, 0, 0);
    this.eyes.scale.set(1, 1, 1);
    this.bulb.scale.setScalar(1 + Math.sin(this._time * 4) * 0.12);

    switch (s.name) {
      case 'idle': {
        this.rig.position.y = Math.sin(this._time * 2) * 0.012;
        this.armL.rotation.x = Math.sin(this._time * 2) * 0.05;
        this.armR.rotation.x = -Math.sin(this._time * 2) * 0.05;
        // A blink every few seconds.
        if (this._time % 3.2 < 0.12) this.eyes.scale.y = 0.15;
        break;
      }
      case 'walk': {
        const swing = Math.sin(this._time * 9);
        this.legL.rotation.x = swing * 0.75;
        this.legR.rotation.x = -swing * 0.75;
        this.armL.rotation.x = -swing * 0.6;
        this.armR.rotation.x = swing * 0.6;
        this.rig.position.y = Math.abs(Math.cos(this._time * 9)) * 0.045;
        this.rig.rotation.z = swing * 0.03;
        break;
      }
      case 'jump': {
        const p = Math.min(t / 0.7, 1);
        this.rig.position.y = Math.sin(Math.PI * p) * 0.55;
        this.legL.rotation.x = -0.5 * Math.sin(Math.PI * p);
        this.legR.rotation.x = -0.5 * Math.sin(Math.PI * p);
        this.armL.rotation.z = -1.1 * Math.sin(Math.PI * p);
        this.armR.rotation.z = 1.1 * Math.sin(Math.PI * p);
        this.eyes.scale.y = 1.25;
        if (p >= 1 && !s.loop) this.play('idle');
        break;
      }
      case 'celebrate': {
        const hop = Math.abs(Math.sin(t * 7));
        this.rig.position.y = hop * 0.22;
        this.rig.rotation.y = this._facing + (s.loop ? t * 3 : Math.min(t / 1.4, 1) * Math.PI * 2);
        this.armL.rotation.z = -2.6 + Math.sin(t * 14) * 0.25;
        this.armR.rotation.z = 2.6 - Math.sin(t * 14) * 0.25;
        this.eyes.scale.y = 0.55;
        this.bulb.scale.setScalar(1.5 + Math.sin(t * 16) * 0.3);
        if (t > 1.6 && !s.loop) this.play('idle');
        break;
      }
      case 'sad': {
        this.head.rotation.x = 0.35;
        this.rig.position.y = -0.02;
        this.armL.rotation.x = 0.15;
        this.armR.rotation.x = 0.15;
        this.eyes.scale.y = 0.6;
        this.bulb.scale.setScalar(0.7);
        break;
      }
      case 'think': {
        this.armR.rotation.x = -2.5;
        this.armR.rotation.z = 0.25;
        this.head.rotation.z = Math.sin(this._time * 1.4) * 0.12;
        this.head.rotation.y = Math.sin(this._time * 0.9) * 0.2;
        this.bulb.scale.setScalar(1 + Math.abs(Math.sin(this._time * 6)) * 0.6);
        break;
      }
      default:
        break;
    }
  }
}

function torus3(r) {
  return new THREE.TorusGeometry(r, 0.02, 6, 12);
}

export const createBot = (kind = 'zirk', options) => new Bot(kind, options);

/** A translucent copy of a bot: what `clone()` produces in the lessons. */
export const createBotGhost = (kind = 'zirk', tint = PALETTE.cyan) => new Bot(kind, { ghosted: true, tint });
