import * as utils from './namespace-canonicalization/utils';
import * as reexp from './namespace-canonicalization/reexport';

console.log(utils.someNamespace.someFunction() + 1);
console.log(reexp.someNamespace.someFunction() + 1);
