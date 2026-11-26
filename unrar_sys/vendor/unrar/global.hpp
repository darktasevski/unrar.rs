#ifndef _RAR_GLOBAL_
#define _RAR_GLOBAL_

#ifdef INCLUDEGLOBAL
  #define EXTVAR
#else
  #define EXTVAR extern
#endif

#ifdef RARDLL
EXTVAR thread_local ErrorHandler ErrHandler;

// A client callback can enter the DLL again on the same thread.
class DllErrorScope
{
  ErrorHandler Previous;
public:
  DllErrorScope():Previous(ErrHandler) { ErrHandler.Clean(); }
  ~DllErrorScope() { ErrHandler=Previous; }
  DllErrorScope(const DllErrorScope &)=delete;
  DllErrorScope &operator=(const DllErrorScope &)=delete;
};
#else
EXTVAR ErrorHandler ErrHandler;
#endif



#endif
