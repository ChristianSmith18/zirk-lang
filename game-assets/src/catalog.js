import { PALETTE } from './palette.js';
import { BOT_KINDS, createBot, createBotGhost } from './characters/bot.js';
import { createCoin, createGem, createKey, createBattery, createChest, createChip, createOrb } from './items/items.js';
import { createGoalPortal, createFlag, createStartPad, createCheckpoint, createDoor, createPlate, createLever, createTerminal } from './goals/goals.js';
import { createTile, TILE_TYPES } from './world/tiles.js';
import { createWall, createPillar, createCrate, createTree, createPine, createRock, createCrystal, createLamp, createSign, createFence, createBush, createBanner } from './props/props.js';
import { createSparkles, createShadowBlob, createHighlightRing, createPathArrow, createTrailDot } from './effects/effects.js';
import { createBlueprint, createCloner, createContractScroll, createObjectPanel, createReferenceLink } from './teaching/teaching.js';

/**
 * Every asset, in one table. `create(options)` returns a THREE.Group; assets
 * with motion expose `update(dt)`. `footprint` is the [x, z] size in tiles,
 * used by the preview and by level editors to lay assets out.
 */
export const CATEGORIES = Object.freeze(['characters', 'items', 'goals', 'world', 'props', 'effects', 'teaching']);

const entry = (category, id, label, create, footprint = [1, 1], notes = '') => ({ category, id, label, create, footprint, notes });

export const CATALOG = Object.freeze([
  ...Object.entries(BOT_KINDS).map(([kind, look]) =>
    entry('characters', `bot-${kind}`, look.label, (o) => createBot(kind, o), [1, 1], 'play(idle|walk|jump|celebrate|sad|think), face(angle)'),
  ),
  entry('characters', 'bot-ghost', 'Clone ghost', (o) => createBotGhost('zirk', o?.tint), [1, 1], 'translucent copy produced by clone()'),

  entry('items', 'coin', 'Coin', createCoin, [1, 1], 'spins and hovers'),
  entry('items', 'gem', 'Gem', createGem, [1, 1], 'worth more than a coin'),
  entry('items', 'key', 'Key', createKey, [1, 1], 'opens a door'),
  entry('items', 'battery', 'Battery', createBattery, [1, 1], 'setLevel(0..1)'),
  entry('items', 'chest', 'Chest', createChest, [1, 1], 'open(0..1)'),
  entry('items', 'chip', 'Data chip', createChip, [1, 1], 'a value token'),
  entry('items', 'orb', 'Life orb', createOrb, [1, 1], 'lives / hearts'),

  entry('goals', 'goal-portal', 'Goal portal', createGoalPortal, [1, 1], 'level exit'),
  entry('goals', 'flag', 'Goal flag', createFlag, [1, 1], 'raise(0..1)'),
  entry('goals', 'start-pad', 'Start pad', createStartPad, [1, 1], 'spawn point'),
  entry('goals', 'checkpoint', 'Checkpoint', createCheckpoint, [1, 1], 'activate(on)'),
  entry('goals', 'door', 'Door', createDoor, [1, 1], 'open(0..1)'),
  entry('goals', 'plate', 'Pressure plate', createPlate, [1, 1], 'press(on)'),
  entry('goals', 'lever', 'Lever', createLever, [1, 1], 'pull(0..1)'),
  entry('goals', 'terminal', 'Terminal', createTerminal, [1, 1], 'blinking cursor'),

  ...TILE_TYPES.map((type) => entry('world', `tile-${type}`, `Tile: ${type}`, (o) => createTile(type, o), [1, 1])),
  entry('world', 'wall', 'Wall', createWall, [1, 1]),
  entry('world', 'pillar', 'Pillar', createPillar, [1, 1]),

  entry('props', 'crate', 'Crate', createCrate),
  entry('props', 'tree', 'Tree', createTree),
  entry('props', 'pine', 'Pine', createPine),
  entry('props', 'rock', 'Rock', createRock),
  entry('props', 'crystal', 'Crystal', createCrystal),
  entry('props', 'lamp', 'Lamp', createLamp),
  entry('props', 'sign', 'Signpost', createSign),
  entry('props', 'fence', 'Fence', createFence),
  entry('props', 'bush', 'Bush', createBush),
  entry('props', 'banner', 'Banner', createBanner),

  entry('effects', 'sparkles', 'Sparkles', createSparkles, [1, 1], 'one-shot; finished flag'),
  entry('effects', 'shadow-blob', 'Shadow blob', createShadowBlob),
  entry('effects', 'highlight-ring', 'Highlight ring', createHighlightRing),
  entry('effects', 'path-arrow', 'Path arrow', createPathArrow),
  entry('effects', 'trail-dot', 'Trail dot', createTrailDot),

  entry('teaching', 'blueprint', 'Blueprint (class)', createBlueprint, [1, 1], 'a class is a plan'),
  entry('teaching', 'cloner', 'Cloner (clone())', createCloner, [3, 1], 'setProgress(0..1)'),
  entry('teaching', 'contract-scroll', 'Contract (interface)', createContractScroll, [1, 1], 'unroll(0..1)'),
  entry('teaching', 'object-panel', 'Object panel (fields)', (o) => createObjectPanel({ fields: 3, ...o }), [1, 1], 'setField(index, ratio)'),
  entry('teaching', 'reference-link', 'Reference link', createReferenceLink, [1, 1], 'alias / identity'),
]);

export const byId = (id) => {
  const found = CATALOG.find((asset) => asset.id === id);
  if (!found) throw new Error(`unknown asset "${id}"`);
  return found;
};

export const byCategory = (category) => CATALOG.filter((asset) => asset.category === category);

/** Instantiate an asset by id. */
export const createAsset = (id, options) => byId(id).create(options);

export { PALETTE };
