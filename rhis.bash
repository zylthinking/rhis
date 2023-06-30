#!/bin/bash

if [[ -t 0 ]] && [[ "$_RHIS_LOADED" != "loaded" ]]; then
  __RHIS_LOADED="loaded"

  if [[ ! -r $HISTFILE  || ! -w $HISTFILE ]]; then
    echo "bash history fullpath need be set into HISTFILE env, and should be readable && writeable."
    return 1
  fi

  EXEUTABLE=$(command which rhis)
  if [ -z "$EXEUTABLE" ]; then
    echo "rhis does not found"
    return 1
  fi

  # Ignore commands with a leading space
  export HISTCONTROL="${HISTCONTROL:-ignorespace}"
  # Append new history items to .bash_history
  shopt -s histappend

  SID="__sid_place_holder__"
  OLDIR=$(pwd)
  IDX=0
  function rhis_prompt_command {
      local exit_code=$?

      local cmd=$(HISTTIMEFORMAT= history 1)
      cmd="${cmd##*( )}"

      local i=${cmd/ */}
      if [ 5$IDX -ne 5$i ]
      then
          if [ $IDX -ne 0 ]
          then
              cmd="${cmd#* }"
              cmd="${cmd##*( )}"
              HISTFILE=$HISTFILE $EXEUTABLE --sid $SID add --dir $OLDIR --exit ${exit_code} "$cmd"
          fi
          IDX=$i
      fi

      OLDIR=$(pwd)
      return ${exit_code}
  }

  if [ -z "$PROMPT_COMMAND" ]
  then
    PROMPT_COMMAND="rhis_prompt_command"
  elif [[ ! "$PROMPT_COMMAND" =~ "rhis_prompt_command" ]]
  then
    PROMPT_COMMAND="rhis_prompt_command;${PROMPT_COMMAND#;}"
  fi

  function rhis_search {
      local cmd=${READLINE_LINE[@]};
      READLINE_LINE= ;
      HISTFILE=$HISTFILE $EXEUTABLE --sid $SID search --dir $OLDIR --light --bottom "$cmd"
  }

  # If this is an interactive shell, take ownership of ctrl-r.
  if [[ $- =~ .*i.* ]]; then
      bind -x '"\C-r": "rhis_search"'
  fi
fi
