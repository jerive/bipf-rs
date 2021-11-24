let bipf = require('../index.node');
let pkg = require('./bipf-package.json');
let binary = require('bipf');

let Benchmark = require('benchmark');

var N = 10000
const encoded = bipf.encode(pkg);

var suite = new Benchmark.Suite;
var dependencies = Buffer.from('dependencies')
var varint = Buffer.from('varint')
suite
.add(function() {
  bipf.encode(pkg)
}, { name: "neon.binary.encode"})
.add(function() {
  let buf = Buffer.allocUnsafe(binary.encodingLength(pkg))
  binary.encode(pkg, buf, 0)
}, { name: "binary.encode"})
.add(function() {
  bipf.decode(encoded)
}, {name: "neon.binary.decode"})
.add(function() {
  binary.decode(encoded, 0)
}, {name: "binary.decode"})
.add(function (){
  bipf.decode(
    encoded,
    bipf.seekKey(encoded, bipf.seekKey(encoded, 0, 'dependencies'), 'varint')
  )
}, {name: "neon.binary.seek(string)"})
.add(function (){
  binary.decode(
    encoded,
    binary.seekKey(encoded, binary.seekKey(encoded, 0, 'dependencies'), 'varint')
  )
}, {name: "binary.seek(string)"})
.add(function () {
  bipf.decode(
    encoded,
    bipf.seekKey(encoded, bipf.seekKey(encoded, 0, dependencies), varint)
  )
}, { name: "neon.binary.seek(buffer)"})
.add(function () {
  // var c, d;
  binary.decode(
    encoded,
    binary.seekKey(encoded, binary.seekKey(encoded, 0, dependencies), varint)
  )
}, { name: "binary.seek(buffer)"})
.on('cycle', function(event) {
  console.log(String(event.target));
})
.run();
