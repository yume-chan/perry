function a(v: number) {
  let fact = (n: number): number => n <= 1 ? 1 : n * fact(n - 1);
  return fact(v);
}

console.log(a(5));
