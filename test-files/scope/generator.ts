export function* mapIterator<T, U>(iter: Iterable<T>, mapFn: (x: T) => U): Generator<U, void, unknown> {
  for (const x of iter) {
    yield mapFn(x);
  }
}

for (const x of mapIterator([1, 2, 3], x => x * 2)) {
  console.log(x);
}
