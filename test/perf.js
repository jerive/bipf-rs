let bipf = require('../index.node');
let bipfReference = require('bipf');
let pkg = require('./bipf-package.json');

var N = 10000
const encoded = bipf.encode(pkg);
var b = Buffer.alloc(bipf.encodingLength(pkg))
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

// ---
start = Date.now()
for (var i = 0; i < N; i++) {
    bipf.decode(
    b,
    bipf.seekKey(b, bipf.seekKey(b, 0, 'dependencies'), 'varint')
  )
}
console.log('binary.seek(string)', N / (Date.now() - start))

start = Date.now()
var dependencies = Buffer.from('dependencies')
var varint = Buffer.from('varint')
for (var i = 0; i < N; i++) {
  var c, d
  bipf.decode(
    b,
    (d = bipf.seekKey(b, (c = bipf.seekKey(b, 0, dependencies)), varint))
  )
}
console.log('binary.seek(buffer)', N / (Date.now() - start))