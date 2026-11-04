import Base
import ./json.bend as J

# Ordered JSON object updates, independent of any client or renderer.

def Obj.drop.hit(+fs: J.Json, +key: String) -> Bool:
  match fs:
    case J.Field{+k, _, _}: String.eq(k, key)
    case _: False{}

def Obj.drop.go(+fs: J.Json, +key: String, +hit: Bool) -> J.Json:
  match fs hit:
    case J.Field{_, _, +tail} True{}: Obj.drop.go(tail, key, Obj.drop.hit(tail, key))
    case J.Field{+k, +v, +tail} False{}: J.Field{k, v, Obj.drop.go(tail, key, Obj.drop.hit(tail, key))}
    case _ _: J.End{}

def Obj.drop(+fs: J.Json, +key: String) -> J.Json:
  Obj.drop.go(fs, key, Obj.drop.hit(fs, key))

def Obj.put(+j: J.Json, +key: String, +value: J.Json) -> J.Json:
  match j:
    case J.Obj{+fs}: J.Obj{J.Field{key, value, Obj.drop(fs, key)}}
    case _: J.Json.obj([J.KV{key, value}])
