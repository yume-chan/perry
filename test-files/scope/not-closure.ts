function getModuleInstanceState(visited?: Map<number, number | undefined>): number {
  return getModuleInstanceStateCached(visited)
}

function getModuleInstanceStateCached(visited = new Map<number, number | undefined>()) {
  const nodeId = Math.random();
  if (visited.has(nodeId)) {
    return visited.get(nodeId)!;
  }
  visited.set(nodeId, undefined);
  const result = 42
  visited.set(nodeId, result);
  return result
}

const f = (visited?: Map<number, number | undefined>) => true ? getModuleInstanceStateCached(visited) : 0;

console.log(getModuleInstanceState(), 'should be 42');
console.log(f(), 'should be 42');
