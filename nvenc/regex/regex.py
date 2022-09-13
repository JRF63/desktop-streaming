import re

pattern = re.compile(r'pub type (\S+) = .+\n.+fn\(\n.+encoder.+,([^\)]+)')
param_pattern = re.compile(r'(\S+):\s+(.+),')
pcase = re.compile(r'[a-zA-Z][a-z]+')

known_types = ['u32', 'u64', 'i32', 'i64', '::std::os::raw::c_int']
for typ in known_types[:]:
    known_types.append(f'*mut {typ}')

def build_mapping():
    pattern = re.compile(r'pub (nvEnc\S+):\s+(\S+),')
    
    table = None
    with open('fntable.txt') as f:
        table = f.read()

    d = {}
    for m in pattern.finditer(table):
        d[m.group(2)] = m.group(1)
    
    # these have the wrong types
    d['PNVENCGETENCODEPROFILEGUIDCOUNT'] = 'nvEncGetEncodeProfileGUIDCount'
    d['PNVENCGETENCODEPROFILEGUIDS'] = 'nvEncGetEncodeProfileGUIDs'
    return d

def rustify_name(name):
    name = name.replace('UID', 'uid')
    return '_'.join(x.lower() for x in pcase.findall(name))

def qualify_type(typ):
    if typ in known_types:
        return typ
    else:
        s = typ.split()
        s[-1] = f'crate::sys::{s[-1]}'
        return ' '.join(s)

mapping = build_mapping()

sigs = None
with open('signatures.txt') as f:
    sigs = f.read()

for m in pattern.finditer(sigs):
    params = [x.strip() for x in m.group(2).strip().split('\n')]

    ptr_type = m.group(1)
    
    member_name = mapping[ptr_type]
    fn_name = rustify_name(member_name[5:]) # strip nvEnc
    print('#[inline(always)]')
    print(f'pub(crate) unsafe fn {fn_name}(')
    print('    &self,')

    args = []
    for param in params:
        n = param_pattern.search(param)
        if n:
            var_name = n.group(1)
            var_name = rustify_name(var_name)
            typ = n.group(2)
            typ = qualify_type(typ)
            print(f'    {var_name}: {typ},')
            args.append(var_name)
    print(') -> Result<()> {')
    print(f'    let status = (self.functions.{member_name}.unwrap_unchecked())(')
    print(f'        self.encoder_ptr.as_ptr(),')
    for arg in args:
        print(f'        {arg},')
    print(f'    );')
    print('    match NvEncError::from_nvenc_status(status) {')
    print('        None => Ok(()),')
    print('        Some(err) => Err(err),')
    print('    }')
    print('}')