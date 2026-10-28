import struct, sys

path = sys.argv[1]
d = open(path, 'rb').read()
pe = d.find(b'PE\x00\x00')
coff = pe + 4
magic = struct.unpack_from('<H', d, coff + 20)[0]
nsec = struct.unpack_from('<H', d, coff + 2)[0]
opt_size = struct.unpack_from('<H', d, coff + 16)[0]
opt = coff + 20
opt = coff + 20
dirs_at = opt + (112 if magic == 0x20b else 96)
dirs = struct.unpack_from('<16I', d, dirs_at)
edata_rva, edata_size = dirs[0], dirs[1]
off = coff + 20 + opt_size
sec_off = off
secs = []
for i in range(nsec):
    s = d[sec_off + i * 40: sec_off + i * 40 + 40]
    vsize, rva, rawsize, rawptr = struct.unpack_from('<4I', s, 8)
    secs.append((rva, rawsize, rawptr))

def rva_to_off(rva):
    for rva_s, rawsize, rawptr in secs:
        if rawsize and rva_s <= rva < rva_s + rawsize:
            return rawptr + (rva - rva_s)
    # file may not have raw data for this rva (pure virtual)
    return None

def read_cstr(o):
    s = b''
    while o < len(d) and d[o] != 0:
        s += d[o:o + 1]
        o += 1
    return s.decode('latin1')

print('nsec', nsec, 'edata_rva', edata_rva, 'edata_size', edata_size)
eoff = rva_to_off(edata_rva)
print('eoff', eoff)
if eoff is None:
    sys.exit(1)
ed = d[eoff:eoff + edata_size]
vals = struct.unpack('<11I', ed[:44])
# data_rva, edata_size, flags, dll_name_rva, serial, version,
# export_count, name_count, addr_table_rva, names_rva, ordinals_rva
export_count, name_count = vals[6], vals[7]
print('export_count', export_count, 'name_count', name_count)
no = rva_to_off(vals[9])
names = []
for i in range(name_count):
    nrva = struct.unpack_from('<I', d, no + i * 4)[0]
    o = rva_to_off(nrva)
    if o is None:
        continue
    names.append(read_cstr(o))
ovr = [n for n in names if n.startswith('ovr')]
print('named total', len(names), 'ovr', len(ovr))
for n in names[:10]:
    print(' sample', n)
sys.argv
