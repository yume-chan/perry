import { exportedFunction } from "./a";

console.log(exportedFunction() + 1);
console.log({ a: exportedFunction }.a() + 1);
