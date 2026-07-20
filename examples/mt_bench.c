#include <stdio.h>
#include <stdint.h>
#include <windows.h>
static uint32_t mt[624]; static int mti=625;
static void seed(uint32_t s){mt[0]=s; for(mti=1;mti<624;mti++) mt[mti]=1812433253U*(mt[mti-1]^(mt[mti-1]>>30))+mti;}
static uint32_t gen(void){uint32_t y,mag01[2]={0,0x9908B0DFU}; int kk;
 if(mti>=624){for(kk=0;kk<227;kk++){y=(mt[kk]&0x80000000U)|(mt[kk+1]&0x7FFFFFFFU);mt[kk]=mt[kk+397]^(y>>1)^mag01[y&1];}
  for(;kk<623;kk++){y=(mt[kk]&0x80000000U)|(mt[kk+1]&0x7FFFFFFFU);mt[kk]=mt[kk-227]^(y>>1)^mag01[y&1];}
  y=(mt[623]&0x80000000U)|(mt[0]&0x7FFFFFFFU);mt[623]=mt[396]^(y>>1)^mag01[y&1];mti=0;}
 y=mt[mti++];y^=y>>11;y^=(y<<7)&0x9D2C5680U;y^=(y<<15)&0xEFC60000U;y^=y>>18;return y;}
int main(){LARGE_INTEGER f,a,b; QueryPerformanceFrequency(&f); QueryPerformanceCounter(&a);
 volatile uint32_t x=0; for(int rep=0;rep<5000;rep++){seed(5489);mti=624; for(int i=0;i<1234;i++) x^=gen();}
 QueryPerformanceCounter(&b); double ms=(b.QuadPart-a.QuadPart)*1000.0/f.QuadPart; printf("C compute 5000x: %.3f ms xor=%u\n", ms, x); return 0;}
