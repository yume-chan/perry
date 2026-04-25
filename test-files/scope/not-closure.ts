export function getModuleInstanceState(visited?: Map<number, number | undefined>): number {
  return Math.random() > 0.5 ? getModuleInstanceStateCached(visited) : 0;
}

function getModuleInstanceStateCached(visited = new Map<number, number | undefined>()) {
  const nodeId = Math.random();
  if (visited.has(nodeId)) {
    return visited.get(nodeId) || 0
  }
  visited.set(nodeId, undefined);
  const result = Math.random();
  visited.set(nodeId, result);
  return result
}
