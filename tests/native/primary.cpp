#include "rar.hpp"

static int CALLBACK CountStreamBytes(UINT Message,LPARAM UserData,LPARAM,LPARAM Length)
{
  if (Message==UCM_PROCESSDATA)
    *reinterpret_cast<int *>(UserData)+=int(Length);
  return 1;
}

// Windows calls ReadSubData from ExtractStreams in test mode. Exercise the
// same native decoder on hosts that do not restore NTFS streams.
extern "C" int native_stream_callback_bytes(const char *Path,int Operation)
{
  CommandData Cmd;
  int CallbackBytes=0;
  Cmd.Callback=CountStreamBytes;
  Cmd.UserData=reinterpret_cast<LPARAM>(&CallbackBytes);
  Cmd.DllOpMode=Operation;
  Archive Arc(&Cmd);
  std::wstring Name;
  UtfToWide(Path,Name);
  if (!Arc.WCheckOpen(Name) || Arc.SearchBlock(HEAD_SERVICE)==0 ||
      !Arc.SubHead.CmpName(SUBHEAD_TYPE_STREAM))
    return -1;
  File Destination;
  if (!Arc.ReadSubData(nullptr,&Destination,true))
    return -2;
  return CallbackBytes;
}
