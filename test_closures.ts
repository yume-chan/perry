function outer(x: number) {
  return () => x + 1;
}

const fn = outer(5);
console.log(fn());
