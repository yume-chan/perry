import { exportedArrowFunction } from "./a";

console.log(exportedArrowFunction() + 1);
console.log({ a: exportedArrowFunction }.a() + 1);
