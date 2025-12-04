import math

def generate(n,size):
    r = size*0.9/2
    tableau = []
    for i in range(n):
        tableau.append([round(size/2+r*math.cos(i*2*math.pi/n)),round(size/2+r*math.sin(i*2*math.pi/n))])
    return tableau