// Shared by the verifier and the level player: the grid simulator that decides
// whether a sequence of "@walk" / "@turn" / "@collect" / "@hop" commands solves
// a map.
const DIRS = { east: [1, 0], south: [0, 1], west: [-1, 0], north: [0, -1] };
const ORDER = ['north', 'east', 'south', 'west'];

export function simulate(level, commands) {
  const map = level.map.map((row) => row.split(''));
  let x = 0;
  let y = 0;
  let coins = 0;
  let total = 0;
  map.forEach((row, ry) => row.forEach((cell, rx) => {
    if (cell === 'S') [x, y] = [rx, ry];
    if (cell === 'c') total++;
  }));
  let facing = level.facing ?? 'east';
  const cellAt = (cx, cy) => map[cy]?.[cx];
  const fail = (why) => ({ ok: false, why, steps: commands.length });
  const move = (dx, dy) => {
    x += dx;
    y += dy;
  };
  for (const command of commands) {
    const [name, arg] = command.slice(1).split(' ');
    if (name === 'walk') {
      const [dx, dy] = DIRS[facing];
      for (let i = 0; i < Number(arg); i++) {
        const next = cellAt(x + dx, y + dy);
        if (next === undefined || next === '#') return fail(`chocaste con un muro en (${x},${y}) mirando al ${facing}`);
        if (next === '~') return fail(`caíste al agua en (${x + dx},${y + dy})`);
        move(dx, dy);
      }
    } else if (name === 'hop') {
      const [dx, dy] = DIRS[facing];
      const landing = cellAt(x + 2 * dx, y + 2 * dy);
      if (landing === undefined || landing === '#' || landing === '~') return fail(`aterrizaste mal en (${x + 2 * dx},${y + 2 * dy})`);
      move(2 * dx, 2 * dy);
    } else if (name === 'turn') {
      const i = ORDER.indexOf(facing);
      facing = ORDER[(i + (arg === 'left' ? 3 : 1)) % 4];
    } else if (name === 'collect') {
      if (cellAt(x, y) !== 'c') return fail(`no hay nada que recoger en (${x},${y})`);
      map[y][x] = '.';
      coins++;
    } else {
      return fail(`orden desconocida ${command}`);
    }
  }
  const need = level.coinsRequired ?? total;
  if (cellAt(x, y) !== 'G') return fail(`terminaste en (${x},${y}), no en la meta`);
  if (coins < need) return fail(`recogiste ${coins} de ${need} monedas`);
  return { ok: true, steps: commands.length, coins };
}

