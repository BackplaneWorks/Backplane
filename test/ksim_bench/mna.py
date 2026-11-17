"""A small linear circuit solver (modified nodal analysis, complex, one frequency at a time), for the benchmark's references: what a task's answer
is, worked out from the component values alone, with no KiCad and no ngspice in the way. refs.py compares it with ngspice on the fixture's netlist.
  c = Ckt(); c.R("in", "out", 1e3); c.C("out", "0", 1e-9); c.V("vin", "in", "0"); v = c.solve(1e3, {"vin": 1})   -> {"out": (0.99-0.006j), ...}"""
import cmath, math

class Ckt:
    def __init__(self): self.el = []; self.nodes = {}; self.br = {}
    def n(self, a):
        if a in ("0", "gnd", "GND"): return -1
        return self.nodes.setdefault(a, len(self.nodes))
    def R(self, a, b, r, name=None): self.el.append(("R", self.n(a), self.n(b), r, name))
    def C(self, a, b, c): self.el.append(("C", self.n(a), self.n(b), c))
    def L(self, a, b, l): self.el.append(("L", self.n(a), self.n(b), l))
    def G(self, p, q, cp, cn, gm): self.el.append(("G", self.n(p), self.n(q), self.n(cp), self.n(cn), gm))  # a current gm*(v(cp)-v(cn)) from p to q through the source
    def E(self, p, q, cp, cn, k): self.br[len(self.br)] = None; self.el.append(("E", self.n(p), self.n(q), self.n(cp), self.n(cn), k, len(self.br) - 1))
    def V(self, name, p, q): self.br[len(self.br)] = name; self.el.append(("V", self.n(p), self.n(q), name, len(self.br) - 1))
    def I(self, name, p, q): self.el.append(("I", self.n(p), self.n(q), name))  # a current from p to q through the source
    def solve(self, f, src):
        w = 2 * math.pi * f; N = len(self.nodes); M = N + len(self.br); A = [[0j] * M for _ in range(M)]; z = [0j] * M
        def y(a, b, g):
            if a >= 0: A[a][a] += g
            if b >= 0: A[b][b] += g
            if a >= 0 and b >= 0: A[a][b] -= g; A[b][a] -= g
        for e in self.el:
            k = e[0]
            if k == "R": y(e[1], e[2], 1 / e[3])
            elif k == "C": y(e[1], e[2], 1j * w * e[3])
            elif k == "L": y(e[1], e[2], 1 / (1j * w * e[3]))
            elif k == "G":
                for row, s in ((e[1], 1), (e[2], -1)):
                    if row < 0: continue
                    if e[3] >= 0: A[row][e[3]] += s * e[5]
                    if e[4] >= 0: A[row][e[4]] -= s * e[5]
            elif k == "I":
                if e[1] >= 0: z[e[1]] -= src.get(e[3], 0)
                if e[2] >= 0: z[e[2]] += src.get(e[3], 0)
            else:  # V, E: a branch current, flowing from p through the source to q
                j = N + e[-1]; p, q2 = e[1], e[2]
                if p >= 0: A[p][j] += 1; A[j][p] += 1
                if q2 >= 0: A[q2][j] -= 1; A[j][q2] -= 1
                if k == "V": z[j] = src.get(e[3], 0)
                else:
                    if e[3] >= 0: A[j][e[3]] -= e[5]
                    if e[4] >= 0: A[j][e[4]] += e[5]
        for i in range(M):  # Gaussian elimination, partial pivoting
            p = max(range(i, M), key=lambda r: abs(A[r][i]))
            if abs(A[p][i]) < 1e-300: raise ZeroDivisionError("singular circuit")
            A[i], A[p] = A[p], A[i]; z[i], z[p] = z[p], z[i]
            for r in range(i + 1, M):
                m = A[r][i] / A[i][i]
                if m:
                    for c in range(i, M): A[r][c] -= m * A[i][c]
                    z[r] -= m * z[i]
        x = [0j] * M
        for i in range(M - 1, -1, -1): x[i] = (z[i] - sum(A[i][c] * x[c] for c in range(i + 1, M))) / A[i][i]
        out = {name: x[i] for name, i in self.nodes.items()}; out["0"] = 0j; return out

def logspace(a, b, per_decade): n = int(round(math.log10(b / a) * per_decade)); return [a * (b / a) ** (i / n) for i in range(n + 1)]
def db(x): return 20 * math.log10(abs(x))
def cross(xs, ys, level, last=False):
    """x where y crosses level (linear in log x between two samples): the first crossing, or the last"""
    hits = [i for i in range(len(xs) - 1) if (ys[i] - level) * (ys[i + 1] - level) <= 0 and ys[i] != ys[i + 1]]
    if not hits: return None
    i = hits[-1] if last else hits[0]; t = (level - ys[i]) / (ys[i + 1] - ys[i])
    return xs[i] * (xs[i + 1] / xs[i]) ** t
def refine(fn, a, b, level, n=60):
    """the x in [a, b] where fn(x) == level (fn monotonic there), by bisection on log x"""
    fa = fn(a) - level
    for _ in range(n):
        m = math.sqrt(a * b); fm = fn(m) - level
        if (fa <= 0) == (fm <= 0): a, fa = m, fm
        else: b = m
    return math.sqrt(a * b)
def margin(fs, T):
    """phase margin in degrees and the crossover frequency of a loop gain T (positive real at low frequency): 180 + its phase where |T| = 1"""
    ph = []; last = 0.0
    for t in T:  # the phase unwrapped along the sweep
        p = math.degrees(cmath.phase(t))
        while p - last > 180: p -= 360
        while p - last < -180: p += 360
        ph.append(p); last = p
    mag = [db(t) for t in T]; i = next((i for i in range(len(fs) - 1) if mag[i] >= 0 > mag[i + 1]), None)
    if i is None: return None, None
    t = (0 - mag[i]) / (mag[i + 1] - mag[i]); return 180 + ph[i] + t * (ph[i + 1] - ph[i]), fs[i] * (fs[i + 1] / fs[i]) ** t
