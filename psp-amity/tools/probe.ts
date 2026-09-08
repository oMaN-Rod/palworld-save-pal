const jsonMode = process.argv.includes("--json");

const dir = `${process.env.LOCALAPPDATA}\\Pal\\Saved\\PSPAmity`;
const ep = JSON.parse(await Bun.file(`${dir}\\endpoint.json`).text());
const ws = new WebSocket(`ws://127.0.0.1:${ep.port}`);

let nextId = 1;
const pending = new Map<string, (env: any) => void>();

function request(type: string, data: Record<string, unknown> = {}): Promise<any> {
  const id = String(nextId++);
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`${type}: timeout`)); }, 10_000);
    pending.set(id, (env) => {
      clearTimeout(t);
      env.type === "error" ? reject(new Error(`${type}: ${env.data.code} ${env.data.message}`)) : resolve(env.data);
    });
    ws.send(JSON.stringify({ id, type, data }));
  });
}

ws.onmessage = (m) => {
  const env = JSON.parse(String(m.data));
  const handler = pending.get(env.id);
  if (handler) {
    pending.delete(env.id);
    handler(env);
  } else if (env.id !== "push") {
    console.error(`notice: unmatched envelope id=${JSON.stringify(env.id)} type=${env.type}`, env.data ?? "");
  }
};

const results: Record<string, unknown> = {};

function report(label: string, data: unknown) {
  results[label] = data;
  if (!jsonMode) console.log(`${label}:`, JSON.stringify(data, null, 2));
}

function command(op: string, args: Record<string, unknown>): Promise<any> {
  return request("command", { commandId: crypto.randomUUID(), op, args });
}

const num = (value: string | undefined) => (value === undefined ? undefined : Number(value));
const list = (value: string | undefined) => (value === undefined ? undefined : value.split(",").filter(Boolean));

async function firstPlayerUid(): Promise<string | undefined> {
  return (await request("get_players")).players?.[0]?.uid;
}

const subcommands: Record<string, { usage: string; run: (argv: string[]) => Promise<[string, unknown]> }> = {
  caps: { usage: "", run: async () => ["capabilities", await request("get_capabilities")] },
  status: { usage: "", run: async () => ["status", await request("get_status")] },
  players: { usage: "", run: async () => ["players", await request("get_players")] },
  pals: {
    usage: "[uid] [page]",
    run: async ([uid, page]) => ["pals", await request("get_pals", { playerUid: uid ?? (await firstPlayerUid()), page: num(page) ?? 0 })],
  },
  detail: {
    usage: "<uid> <slotIndex|instanceId>",
    run: async ([uid, which]) => {
      const args = /^\d+$/.test(which ?? "") ? { slotIndex: num(which) } : { instanceId: which };
      return ["pal_detail", await request("get_pal_detail", { playerUid: uid, ...args })];
    },
  },
  inventory: {
    usage: "[uid]",
    run: async ([uid]) => ["inventory", await request("get_inventory", { playerUid: uid ?? (await firstPlayerUid()) })],
  },
  guild: {
    usage: "[uid]",
    run: async ([uid]) => ["guild", await request("guild", { playerUid: uid ?? (await firstPlayerUid()) })],
  },
  guilds: { usage: "", run: async () => ["guilds", await request("guilds", {})] },
  basepals: { usage: "<baseId>", run: async ([baseId]) => ["base_pals", await request("base_pals", { baseId })] },
  containers: {
    usage: "<guildId>",
    run: async ([guildId]) => ["guild_containers", await request("guild_containers", { guildId })],
  },
  find: { usage: "<name>", run: async ([needle]) => [`find ${needle}`, await request("reflect", { search: needle })] },
  holder: { usage: "<TypeName>", run: async ([needle]) => [`holder ${needle}`, await request("reflect", { holder: needle })] },
  reflect: {
    usage: "<path> [own]",
    run: async ([path, superFlag]) => [`reflect ${path}`, await request("reflect", { path, includeSuper: superFlag !== "own" })],
  },
  heal: {
    usage: "<uid> <slotIndex>",
    run: async ([uid, slot]) => ["pal.heal", await command("pal.heal", { playerUid: uid, slotIndex: num(slot) })],
  },
  setslot: {
    usage: "<uid> <containerId> <slotIndex> [staticItemId] [count]",
    run: async ([uid, containerId, slotIndex, staticItemId, count]) => [
      "item.setSlot",
      await command("item.setSlot", {
        playerUid: uid,
        containerId,
        slotIndex: num(slotIndex),
        staticItemId: staticItemId ?? null,
        count: num(count),
      }),
    ],
  },
  remove: {
    usage: "<uid> <slotIndex>",
    run: async ([uid, slot]) => ["pal.remove", await command("pal.remove", { playerUid: uid, slotIndex: num(slot) })],
  },
  move: {
    usage: "<uid> <fromSlotIndex> <toSlotIndex>",
    run: async ([uid, from, to]) => [
      "pal.move",
      await command("pal.move", { playerUid: uid, fromSlotIndex: num(from), toSlotIndex: num(to) }),
    ],
  },
  add: {
    usage: "<uid> <slotIndex|party|baseId:slotIndex> <characterId> [level] [gender]",
    run: async ([uid, where, characterId, level, gender]) => {
      const colon = where?.indexOf(":") ?? -1;
      const target =
        where === "party"
          ? { party: true }
          : colon > 0
            ? { baseId: where.slice(0, colon), slotIndex: num(where.slice(colon + 1)) }
            : { slotIndex: num(where) };
      return [
        "pal.add",
        await command("pal.add", { playerUid: uid, ...target, characterId, level: num(level), gender }),
      ];
    },
  },
  edit: {
    usage: "<uid> <slotIndex|instanceId> <field=value ...>  (lists comma-separated)",
    run: async ([uid, which, ...fields]) => {
      const args: Record<string, unknown> = /^\d+$/.test(which ?? "") ? { slotIndex: num(which) } : { instanceId: which };
      for (const field of fields) {
        const eq = field.indexOf("=");
        const key = field.slice(0, eq);
        const raw = field.slice(eq + 1);
        args[key] =
          key === "activeSkills" || key === "passiveSkills" ? list(raw)
          : key === "workSuitability" ? Object.fromEntries(raw.split(",").map((kv) => { const [k, v] = kv.split(":"); return [k, Number(v)]; }))
          : key === "nickname" || key === "gender" ? raw
          : raw === "true" || raw === "false" ? raw === "true"
          : Number(raw);
      }
      return ["pal.edit", await command("pal.edit", { playerUid: uid, ...args })];
    },
  },
  level: {
    usage: "<uid> <level> <exp>",
    run: async ([uid, level, exp]) => [
      "player.edit",
      await command("player.edit", { playerUid: uid, level: num(level), exp: num(exp) }),
    ],
  },
  guildlevel: {
    usage: "<guildId> <baseCampLevel>",
    run: async ([guildId, level]) => ["guild.edit", await command("guild.edit", { guildId, baseCampLevel: num(level) })],
  },
  role: {
    usage: "<guildId> <memberUid> <role>",
    run: async ([guildId, memberUid, role]) => ["guild.setRole", await command("guild.setRole", { guildId, memberUid, role })],
  },
};

const name = process.argv[2];
if (name && !subcommands[name]) {
  console.error(`unknown subcommand: ${name}`);
  console.error(Object.entries(subcommands).map(([n, s]) => `  ${n} ${s.usage}`).join("\n"));
  process.exit(2);
}

ws.onopen = async () => {
  try {
    report("hello", await request("hello", { protocolVersion: 1 }));
    await request("auth", { token: ep.token });

    if (name) {
      const args = process.argv.slice(3).filter((a) => !a.startsWith("--"));
      const [label, data] = await subcommands[name].run(args);
      report(label, data);
    } else {
      report("status", await request("get_status"));
      report("players", await request("get_players"));
    }

    if (jsonMode) console.log(JSON.stringify(results));
    process.exit(0);
  } catch (e) {
    if (jsonMode) console.error(JSON.stringify({ error: String(e) }));
    else console.error(String(e));
    process.exit(1);
  }
};

ws.onerror = () => {
  console.error("connect failed");
  process.exit(1);
};
