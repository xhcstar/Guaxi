// 知识图谱力导向布局 Worker
// 把 d3-force-3d 的物理布局计算移出主线程,避免大图布局时卡 UI。
// 协议:主线程 postMessage({ id, nodes:[{id,x?,y?,z?}], links:[{source,target,strength}], ticks })
//       worker  postMessage({ id, ok, points:[{id,x,y,z}] }) 或 { id, ok:false, error }
import {
  forceSimulation,
  forceManyBody,
  forceLink,
  forceCenter,
  forceCollide,
} from "d3-force-3d";

self.onmessage = (event) => {
  const { id, nodes, links, ticks } = event.data || {};
  try {
    const simNodes = (nodes || []).map((n) =>
      n.x != null ? { id: n.id, x: n.x, y: n.y, z: n.z } : { id: n.id }
    );
    const idset = new Set(simNodes.map((n) => n.id));
    const simLinks = (links || [])
      .filter((e) => idset.has(e.source) && idset.has(e.target))
      .map((e) => ({ source: e.source, target: e.target, strength: e.strength ?? 0.3 }));

    const sim = forceSimulation(simNodes, 3)
      .force("charge", forceManyBody().strength(-30))
      .force(
        "link",
        forceLink(simLinks)
          .id((d) => d.id)
          .distance((l) => 18 + (1 - (l.strength ?? 0.3)) * 30)
          .strength((l) => Math.min(1, l.strength ?? 0.3))
      )
      .force("center", forceCenter(0, 0, 0))
      .force("collide", forceCollide(3))
      .stop();

    const n = Math.max(1, simNodes.length);
    const tickCount = ticks || Math.min(300, Math.max(80, Math.round(120 + n / 40)));
    for (let i = 0; i < tickCount; i++) sim.tick();

    const points = simNodes.map((nn) => ({ id: nn.id, x: nn.x || 0, y: nn.y || 0, z: nn.z || 0 }));
    self.postMessage({ id, ok: true, points });
  } catch (error) {
    self.postMessage({ id, ok: false, error: String((error && error.message) || error) });
  }
};
