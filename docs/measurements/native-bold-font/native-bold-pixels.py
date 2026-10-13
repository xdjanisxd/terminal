import pathlib,struct,zlib
root=pathlib.Path('/tmp/native-bold-validation')
def png(path):
    data=path.read_bytes();pos=8;packed=b''
    while pos<len(data):
        n=struct.unpack('>I',data[pos:pos+4])[0];tag=data[pos+4:pos+8];body=data[pos+8:pos+8+n];pos+=12+n
        if tag==b'IHDR':w,h,depth,color,_,_,interlace=struct.unpack('>IIBBBBB',body)
        if tag==b'IDAT':packed+=body
    assert depth==8 and color in [2,6] and interlace==0
    bpp=3 if color==2 else 4;raw=zlib.decompress(packed);rows=[];stride=w*bpp
    prev=bytearray(stride)
    for y in range(h):
        off=y*(stride+1);kind=raw[off];row=bytearray(raw[off+1:off+1+stride])
        for x in range(stride):
            a=row[x-bpp] if x>=bpp else 0;b=prev[x];c=prev[x-bpp] if x>=bpp else 0
            if kind==1:v=a
            elif kind==2:v=b
            elif kind==3:v=(a+b)//2
            elif kind==4:
                p=a+b-c;dist=[abs(p-a),abs(p-b),abs(p-c)];v=[a,b,c][dist.index(min(dist))]
            else:assert kind==0;v=0
            row[x]=(row[x]+v)&255
        rows.append(bytes(row));prev=row
    return w,h,bpp,rows
a=png(root/'baseline-ansi.png');b=png(root/'candidate-ansi.png');assert a[:3]==b[:3]
w,h,bpp,_=a
lines=[]
for label,y0,y1 in [('regular',76,98),('bold',98,120),('reset',120,142),('regular colors',164,186)]:
    same=all(a[3][y][:600*bpp]==b[3][y][:600*bpp] for y in range(y0,y1))
    lines.append(f'{label}: baseline/candidate pixels identical = {same}')
    assert same if label!='bold' else not same
print('\n'.join(lines));(root/'pixel-comparison.txt').write_text('\n'.join(lines)+'\n')
