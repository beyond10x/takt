import os,sys,zlib
g,oid,f,n=sys.argv[1:5]; d=open(f,'rb').read()+b"\n"+b"a"*int(n)
raw=b"commit %d\x00"%len(d)+d
c=zlib.compressobj(9); out=c.compress(raw)+c.flush()
p=os.path.join(g,"objects",oid[:2],oid[2:]); os.chmod(p,0o644); os.remove(p); open(p,'wb').write(out)
print(len(out))
