let bipf = require('./../index.node');
let bipfReference = require('bipf');
let pkg = require('../package.json');
const tape = require('tape')

function testEncodeDecode(value) {
    tape('encode & decode: ' + JSON.stringify(value), (t) => {
        const encoded = bipf.encode(value)
        t.equals(bipf.encodingLength(value), bipfReference.encodingLength(value), "Encoding length must be equal")
        const buf = Buffer.alloc(bipfReference.encodingLength(value))
        // const buf = Buffer.alloc(encoded.length)
        const len = bipfReference.encode(value, buf, 0)
        t.deepEqual(buf, encoded, "reference and neon must be binary perfect");
        // console.log('encoded:', buf.slice(0, len))
        //''+jsonString to get 'undefined' string.
        // const jsonLen = Buffer.byteLength('' + JSON.stringify(value))
        // console.log('length:', len, 'JSON-length:', jsonLen)
        // if (len > jsonLen)
        //     console.log('WARNING: binary encoding longer than json for:', value)
        // if (len === 1) {
        //     const rest = buf[0] >> 3
        //     t.equal(rest, 0, 'single byte encodings must have zero length in tag')
        // }
        t.deepEqual(bipf.decode(encoded, 0), value)
        // t.deepEqual(bipf.decode(buf, 0), value)
        // t.deepEqual(bipf.decode(buf.slice(0, len), 0), value)

        t.end()
    })
  }

testEncodeDecode(100)
testEncodeDecode(0)
testEncodeDecode(1)
testEncodeDecode(-1)
testEncodeDecode(true)
testEncodeDecode(false)
testEncodeDecode(null)
// testEncodeDecode(undefined) // added undefined for compatibility with charwise
testEncodeDecode('')
testEncodeDecode(Buffer.alloc(0))
testEncodeDecode([])
testEncodeDecode([0, 1])
testEncodeDecode({})
testEncodeDecode([1, 2, 3, 4, 5, 6, 7, 8, 9])
testEncodeDecode('hello')
testEncodeDecode({ foo: true })
testEncodeDecode([-1, { foo: true }, Buffer.from('deadbeef', 'hex')])
testEncodeDecode(pkg)
testEncodeDecode(0.1)
