let bipf = require('../index.node');
let bipfReference = require('bipf');
let pkg = require('../package.json');

var N = 10000
const encoded = bipf.encode(pkg);
console.log('operation, ops/ms')
start = Date.now()
for (var i = 0; i < N; i++) {
  //not an honest test
  // b = Buffer.allocUnsafe(binary.encodingLength(value))
  bipf.encode(pkg)
}
console.log('binary.encode', N / (Date.now() - start))

start = Date.now()
for (var i = 0; i < N; i++) {
  //not an honest test
  // b = Buffer.allocUnsafe(binary.encodingLength(value))
  bipf.decode(encoded)
}
console.log('binary.decode', N / (Date.now() - start))
