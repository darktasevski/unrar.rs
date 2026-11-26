#include "rar.hpp"
#include <future>
#include <thread>

static int Read(const char *Path,UNRARCALLBACK Callback,void *Data)
{
  RAROpenArchiveDataEx Open{};
  Open.OpenMode=RAR_OM_EXTRACT;
  Open.ArcName=const_cast<char *>(Path);
  Open.Callback=Callback;
  Open.UserData=reinterpret_cast<LPARAM>(Data);
  HANDLE Handle=RAROpenArchiveEx(&Open);
  if (Handle==nullptr)
    return Open.OpenResult;
  RARHeaderDataEx Header{};
  int Code=Open.OpenResult;
  if (Code==ERAR_SUCCESS)
    Code=RARReadHeaderEx(Handle,&Header);
  if (Code==ERAR_SUCCESS)
    Code=RARProcessFile(Handle,RAR_TEST,nullptr,nullptr);
  int Close=RARCloseArchive(Handle);
  return Close==ERAR_SUCCESS ? Code:Close;
}

struct NestedOpen {
  const char *Path;
  bool SeparateThread;
  const char *Damaged;
  bool Preserved=false;
  bool InnerEntered=false;
  std::promise<void> Entered;
  std::promise<void> Resume;
};

static int CALLBACK InnerRead(UINT Message,LPARAM UserData,LPARAM Bytes,LPARAM Count)
{
  if (Message!=UCM_PROCESSDATA)
    return 1;
  NestedOpen *Test=reinterpret_cast<NestedOpen *>(UserData);
  if (Count!=11 || memcmp(reinterpret_cast<void *>(Bytes),"unrar-0.4.0",11)!=0 ||
      ErrHandler.GetErrorCode()!=RARX_SUCCESS)
    return -1;
  Test->InnerEntered=true;
  ErrHandler.SetErrorCode(RARX_OPEN);
  if (Test->SeparateThread)
  {
    Test->Entered.set_value();
    Test->Resume.get_future().wait();
  }
  return ErrHandler.GetErrorCode()==RARX_OPEN ? 1:-1;
}

static int CALLBACK OuterRead(UINT Message,LPARAM UserData,LPARAM,LPARAM)
{
  if (Message!=UCM_PROCESSDATA)
    return 1;
  NestedOpen *Test=reinterpret_cast<NestedOpen *>(UserData);
  ErrHandler.SetErrorCode(RARX_CRC);
  if (Test->SeparateThread)
  {
    int Code=ERAR_UNKNOWN;
    std::thread Other([&] {
      Code=Read(Test->Path,InnerRead,Test);
      // A callback can be skipped when the native read fails before delivering bytes.
      if (!Test->InnerEntered)
        Test->Entered.set_value();
    });
    Test->Entered.get_future().wait();
    int DamagedResult=Read(Test->Damaged,nullptr,nullptr);
    bool Isolated=DamagedResult==ERAR_BAD_DATA && ErrHandler.GetErrorCode()==RARX_CRC;
    Test->Resume.set_value();
    Other.join();
    Test->Preserved=Test->InnerEntered && Isolated && Code==ERAR_SUCCESS;
  }
  else
    Test->Preserved=Read("Cargo.toml",nullptr,nullptr)==ERAR_BAD_ARCHIVE &&
                    Read(Test->Path,InnerRead,Test)==ERAR_SUCCESS &&
                    Test->InnerEntered && ErrHandler.GetErrorCode()==RARX_CRC;
  return -1;
}

extern "C" int native_nested_open(const char *Path,const char *Damaged)
{
  NestedOpen Test{Path,Damaged!=nullptr,Damaged};
  int Result=Read(Path,OuterRead,&Test);
  return Result==ERAR_UNKNOWN && Test.Preserved ? 0:1;
}

static void Fail(void *Mode)
{
  switch (*static_cast<int *>(Mode))
  {
    case 0: ErrHandler.MemoryError(); break;
    case 1: throw std::bad_alloc();
    case 2: throw 42;
    case 3: ErrHandler.SetErrorCode(RARX_MEMORY); break;
  }
}

static void Increment(void *Count)
{
  ++*static_cast<unsigned *>(Count);
}

struct WorkerTest {
  int Mode;
  unsigned Count=0;
  bool Reused=false;
};

static int CALLBACK FailWorkerDuringRead(UINT Message,LPARAM UserData,LPARAM,LPARAM)
{
  if (Message!=UCM_PROCESSDATA)
    return 1;
  WorkerTest *Test=reinterpret_cast<WorkerTest *>(UserData);
  ThreadPool Pool(1);
  Pool.AddTask(Fail,&Test->Mode);
  Pool.AddTask(Increment,&Test->Count);
  try { Pool.WaitDone(); }
  catch (RAR_EXIT Code)
  {
    if (ErrHandler.GetErrorCode()!=Code || Test->Count!=1)
      return -1;
    Pool.AddTask(Increment,&Test->Count);
    Pool.WaitDone();
    Test->Reused=Test->Count==2;
    throw;
  }
  return 1;
}

extern "C" int native_worker_failure(const char *Path,int Mode)
{
  WorkerTest Test{Mode};
  int Result=Read(Path,FailWorkerDuringRead,&Test);
  int Expected=Mode==2 ? ERAR_EREAD:ERAR_NO_MEMORY;
  return Result==Expected && Test.Reused ? 0:1;
}

extern "C" bool native_abandoned_batch()
{
  unsigned Count=0;
  try
  {
    ThreadPool Pool(1);
    Pool.AddTask(Increment,&Count);
    Pool.WaitDone();
    Pool.AddTask(Increment,&Count);
    throw RARX_READ;
  }
  catch (RAR_EXIT) {}
  return Count==1;
}
