# /etc/zshenv has already run. Restore the user's ZDOTDIR while sourcing each
# user file; keep the wrapper directory only between startup stages.
if (( ${+__TERMINAL_ZDOTDIR} )); then
    export ZDOTDIR=$__TERMINAL_ZDOTDIR
else
    unset ZDOTDIR
fi
__terminal_source_user() {
    local __terminal_file=$1
    if (( ${+__TERMINAL_ZDOTDIR} )); then
        export ZDOTDIR=$__TERMINAL_ZDOTDIR
    else
        unset ZDOTDIR
    fi
    [[ -r ${ZDOTDIR-$HOME}/$__terminal_file ]] && builtin source "${ZDOTDIR-$HOME}/$__terminal_file"
    if (( ${+ZDOTDIR} )); then
        export __TERMINAL_ZDOTDIR=$ZDOTDIR
    else
        unset __TERMINAL_ZDOTDIR
    fi
    export ZDOTDIR=$__TERMINAL_INTEGRATION_DIR
    return 0
}
if [[ $__TERMINAL_NO_RC != 1 ]]; then
    __terminal_source_user .zshenv
else
    unsetopt rcs
fi
builtin source "$__TERMINAL_INTEGRATION_DIR/hook.zsh"
export ZDOTDIR=$__TERMINAL_INTEGRATION_DIR
