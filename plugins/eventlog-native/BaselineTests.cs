using System;
using System.Reflection;
class BaselineTests {
 static string Query(long last,string start){return (string)typeof(Program).GetMethod("EventQuery",BindingFlags.Static|BindingFlags.NonPublic).Invoke(null,new object[]{last,start});}
 static void Check(bool condition){if(!condition)throw new Exception("Event baseline query test failed");}
 static void Main(){
  Check(Query(0,null)=="*");
  Check(Query(42,null)=="*[System[EventRecordID >= 42]]");
  string cutoff="2026-10-22T03:00:00.123Z";
  string normalized="TimeCreated[@SystemTime >= '2026-10-22T03:00:00.1230000Z']";
  Check(Query(0,cutoff)=="*[System["+normalized+"]]");
  Check(Query(42,cutoff)=="*[System[EventRecordID >= 42 and "+normalized+"]]");
  foreach(string bad in new[]{"invalid","2026-10-22T03:00:00Z'] or *[","2026-13-22T03:00:00Z"}){
   bool rejected=false;try{Query(0,bad);}catch(TargetInvocationException){rejected=true;}Check(rejected);
  }
  Console.WriteLine("Event baseline: legacy, initial, resume, reset and invalid cutoff checks passed");
 }
}
