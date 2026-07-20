#include <stdio.h>
#include <stdint.h>
static uint32_t mt[624]; static int mti=625;
static void seed(uint32_t s){mt[0]=s; for(mti=1;mti<624;mti++) mt[mti]=1812433253U*(mt[mti-1]^(mt[mti-1]>>30))+mti;}
static uint32_t gen(void){uint32_t y,mag01[2]={0,0x9908B0DFU}; int kk;
 if(mti>=624){for(kk=0;kk<227;kk++){y=(mt[kk]&0x80000000U)|(mt[kk+1]&0x7FFFFFFFU);mt[kk]=mt[kk+397]^(y>>1)^mag01[y&1];}
  for(;kk<623;kk++){y=(mt[kk]&0x80000000U)|(mt[kk+1]&0x7FFFFFFFU);mt[kk]=mt[kk-227]^(y>>1)^mag01[y&1];}
  y=(mt[623]&0x80000000U)|(mt[0]&0x7FFFFFFFU);mt[623]=mt[396]^(y>>1)^mag01[y&1];mti=0;}
 y=mt[mti++];y^=y>>11;y^=(y<<7)&0x9D2C5680U;y^=(y<<15)&0xEFC60000U;y^=y>>18;return y;}
static long long fib(int n){long long a=0,b=1; if(n<=1)return n; for(int i=2;i<=n;i++){long long t=a+b;a=b;b=t;} return b;}
int main(){seed(5489); uint32_t x=0,s=0,last=0; for(int i=0;i<1234;i++){last=gen();x^=last;s+=last;}
 printf("%u\n%u\n%u\n",x,s,last); printf("%lld\n",fib(90)); return 0;}
